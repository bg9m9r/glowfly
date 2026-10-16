//! External programs the user chose (Preferences > External Tools): splitting an argument
//! template into arguments, filling in its placeholders, finding the program on PATH, and
//! starting it detached.
//!
//! Templates are split before placeholders are filled in, so a path with spaces is always one
//! argument, and nothing goes through a shell. Whitespace separates arguments and double quotes
//! group text (the quotes themselves are dropped); there is no escape character, so backslashes
//! in Windows paths are taken literally.

use crate::error::{Error, Result};
use crate::native_path::native;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Why a template can't be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TemplateError {
    #[error("unmatched quote")]
    UnmatchedQuote,
}

/// The placeholders a template may name.
pub const PLACEHOLDERS: [&str; 3] = ["{path}", "{line}", "{repo}"];

/// Splits `template` into arguments: whitespace separates them, double quotes group text with
/// spaces (`a"b c"d` is `ab cd`, `""` is one empty argument), and backslashes are plain text.
pub fn split(template: &str) -> Result<Vec<String>, TemplateError> {
    let mut out = Vec::new();
    let mut current = String::new();
    // Whether an argument has started (a quote starts one even if it stays empty).
    let mut started = false;
    let mut quoted = false;
    for ch in template.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(TemplateError::UnmatchedQuote);
    }
    if started {
        out.push(current);
    }
    Ok(out)
}

/// What the placeholders stand for. For repository and terminal templates `path` is the
/// repository folder and `line` is 1.
#[derive(Clone, Copy, Debug)]
pub struct Vars<'a> {
    /// `{path}`: the file's full path.
    pub path: &'a Path,
    /// `{line}`: the line number.
    pub line: u32,
    /// `{repo}`: the repository folder.
    pub repo: &'a Path,
}

/// The arguments of `template` with its placeholders filled in from `v`, in one pass (a value
/// that itself contains `{line}` is not expanded again). Unknown `{names}` are kept as typed.
pub fn expand(template: &str, v: &Vars) -> Result<Vec<OsString>, TemplateError> {
/// Paths go in in this system's spelling (`C:\x\y`, never libgit2's `C:/x/y/`).
    Ok(split(template)?.iter().map(|a| substitute(a, v)).collect())
}

fn substitute(arg: &str, v: &Vars) -> OsString {
    let mut out = OsString::new();
    let mut rest = arg;
    while let Some(i) = rest.find('{') {
        out.push(&rest[..i]);
        let at = &rest[i..];
        if let Some(r) = at.strip_prefix("{path}") {
            out.push(native(v.path));
            rest = r;
        } else if let Some(r) = at.strip_prefix("{line}") {
            out.push(v.line.to_string());
            rest = r;
        } else if let Some(r) = at.strip_prefix("{repo}") {
            out.push(native(v.repo));
            rest = r;
        } else {
            out.push("{");
            rest = &at[1..];
        }
    }
    out.push(rest);
    out
}

/// Each `{name}` in `template` that is not a placeholder, once each, in the order they appear.
pub fn unknown_placeholders(template: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        rest = &rest[i + 1..];
        let name_len = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        if name_len > 0 && rest[name_len..].starts_with('}') {
            let word = format!("{{{}}}", &rest[..name_len]);
            if !PLACEHOLDERS.contains(&word.as_str()) && !out.contains(&word) {
                out.push(word);
            }
        }
    }
    out
}

/// The program named by a Preferences field: trimmed, without one pair of surrounding quotes
/// (as pasted from a file manager); None when nothing is set.
pub fn program(text: &str) -> Option<PathBuf> {
    let t = text.trim();
    let t = t.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(t);
    (!t.trim().is_empty()).then(|| PathBuf::from(t))
}

/// Windows' usual PATHEXT, for when the variable is unset.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Where `program` is: a path (more than one component, or absolute) if it `exists`, else the
/// first folder of `path_var` holding it. With `windows`, a bare name without an extension is
/// tried with each of `pathext`'s extensions (so `code` finds `code.cmd`). Only PATH is
/// searched, never the current folder.
pub fn resolve(
    program: &Path,
    path_var: Option<&OsStr>,
    pathext: Option<&OsStr>,
    windows: bool,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if program.is_absolute() || program.components().count() > 1 {
        return exists(program).then(|| program.to_owned());
    }
    let exts: Vec<String> = if windows && program.extension().is_none() {
        let list = pathext.and_then(OsStr::to_str).filter(|s| !s.trim().is_empty()).unwrap_or(DEFAULT_PATHEXT);
        list.split(';').map(str::trim).filter(|e| !e.is_empty()).map(str::to_owned).collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(path_var?) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for ext in &exts {
            let mut name = program.as_os_str().to_owned();
            name.push(ext);
            let candidate = dir.join(name);
            if exists(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// A program to start: `program` as the user typed it (resolved by `spawn`), its expanded
/// arguments and the folder it starts in.
#[derive(Clone, Debug, PartialEq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    /// A terminal: started in a console window of its own, keeping that console's input and
    /// output, so a console program (cmd, PowerShell, a shell) is usable.
    pub own_console: bool,
}

fn has_extension(p: &Path, ext: &str) -> bool {
    p.extension().and_then(OsStr::to_str).is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// The command line that starts `resolved` with `args`: on macOS an application bundle is
/// started through `open -a`, everything else as is.
pub(crate) fn command_line(resolved: &Path, args: &[OsString], macos: bool) -> (PathBuf, Vec<OsString>) {
    if macos && has_extension(resolved, "app") {
        let mut all = vec![OsString::from("-a"), resolved.as_os_str().to_owned()];
        all.extend(args.iter().cloned());
        return (PathBuf::from("open"), all);
    }
    (resolved.to_owned(), args.to_vec())
}

/// How a launch is attached to consoles.
#[derive(Debug, PartialEq)]
pub(crate) struct ConsoleSetup {
    /// Standard input, output and error go to the null device.
    pub null_stdio: bool,
    /// Windows process-creation flags.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub creation_flags: u32,
}

const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The console setup for a launch. An editor gets no input or output, and a batch-file wrapper
/// (such as an editor's command-line script) gets no window, which would only flash. A terminal
/// keeps the inherited handles and, on Windows, gets a new console: Windows then gives it that
/// console's handles. Nulled handles would stay pointed at NUL, so cmd would read end of input
/// and quit at once, and PowerShell would print nowhere.
pub(crate) fn console_setup(own_console: bool, batch_file: bool) -> ConsoleSetup {
    match (own_console, batch_file) {
        (true, _) => ConsoleSetup { null_stdio: false, creation_flags: CREATE_NEW_CONSOLE },
        (false, true) => ConsoleSetup { null_stdio: true, creation_flags: CREATE_NO_WINDOW },
        (false, false) => ConsoleSetup { null_stdio: true, creation_flags: 0 },
    }
}

/// Starts `l` without waiting for it, reaped on a background thread; see `console_setup` for
/// its input and output. `Error::NotFound` when the program can't be found.
pub fn spawn(l: &Launch) -> Result<()> {
    let macos = cfg!(target_os = "macos");
    let exists = |p: &Path| p.is_file() || (macos && has_extension(p, "app") && p.is_dir());
    let path_var = std::env::var_os("PATH");
    let pathext = std::env::var_os("PATHEXT");
    let resolved = resolve(&l.program, path_var.as_deref(), pathext.as_deref(), cfg!(windows), &exists)
        .ok_or_else(|| Error::NotFound(l.program.display().to_string()))?;
    let (program, args) = command_line(&resolved, &l.args, macos);
    let mut cmd = Command::new(&program);
    cmd.args(&args).current_dir(&l.cwd);
    let batch = cfg!(windows) && (has_extension(&resolved, "cmd") || has_extension(&resolved, "bat"));
    let setup = console_setup(l.own_console, batch);
    if setup.null_stdio {
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(setup.creation_flags);
    }
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Error::NotFound(l.program.display().to_string()),
        _ => Error::Io(e),
    })?;
    // Reaped so it leaves no zombie on Unix; Trellis never waits for the program.
    let _ = std::thread::Builder::new().name("trellis-tool-wait".into()).spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests;
