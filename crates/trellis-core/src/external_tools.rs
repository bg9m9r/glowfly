//! External programs the user chose (Preferences > External Tools): argument templates
//! (`template`), finding the program on PATH, and starting it, detached or waiting for it to
//! exit. `temp` holds the private folder for the copies diff and merge tools are given, and
//! `compare` writes those copies for a diff or a conflict.

use crate::error::{Error, Result};
use crate::native_path::native;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub mod compare;
pub mod temp;
mod template;
pub use template::*;

/// The program named by a Preferences field: trimmed, without one pair of surrounding quotes
/// (as pasted from a file manager); None when nothing is set.
pub fn program(text: &str) -> Option<PathBuf> {
    let t = text.trim();
    let t = t.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(t);
    (!t.trim().is_empty()).then(|| PathBuf::from(t))
}

/// Windows' usual PATHEXT, for when the variable is unset.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Where `program` is: a path (more than one component, or absolute) if `find` finds it, else
/// the first folder of `path_var` holding it. With `windows`, a bare name without an extension
/// is tried with each of `pathext`'s extensions (so `code` finds `code.cmd`). Only PATH is
/// searched, never the current folder. `find` answers where a candidate really is (see
/// [`locate`]), or None.
pub fn resolve(
    program: &Path,
    path_var: Option<&OsStr>,
    pathext: Option<&OsStr>,
    windows: bool,
    find: &dyn Fn(&Path) -> Option<PathBuf>,
) -> Option<PathBuf> {
    if program.is_absolute() || program.components().count() > 1 {
        return find(program);
    }
    let names = candidate_names(program, pathext, windows);
    for dir in std::env::split_paths(path_var?) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for name in &names {
            if let Some(found) = find(&dir.join(name)) {
                return Some(found);
            }
        }
    }
    None
}

/// The file names a PATH search tries in each folder for the bare name `program`: with
/// `windows` and no extension, `program` plus each of `pathext`'s extensions (Windows' usual
/// list when it is unset or empty); otherwise `program` itself.
pub(crate) fn candidate_names(program: &Path, pathext: Option<&OsStr>, windows: bool) -> Vec<OsString> {
    if !(windows && program.extension().is_none()) {
        return vec![program.as_os_str().to_owned()];
    }
    let list = pathext.and_then(OsStr::to_str).filter(|s| !s.trim().is_empty()).unwrap_or(DEFAULT_PATHEXT);
    list.split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|ext| {
            let mut name = program.as_os_str().to_owned();
            name.push(ext);
            name
        })
        .collect()
}

/// Where the program file `p` is, as a PATH search should see it:
/// - with `windows`, names match whatever their case, by listing the folder when the exact
///   name isn't there (a WSL folder, or one with per-directory case sensitivity, keeps
///   `Code.exe` and `code.exe` apart);
/// - on Unix (without `windows`), only a file with an executable bit counts, as for a shell;
/// - with `macos`, an application bundle (a folder ending in `.app`) counts too.
pub fn locate(p: &Path, windows: bool, macos: bool) -> Option<PathBuf> {
    if let Ok(m) = std::fs::metadata(p) {
        if m.is_file() && (windows || executable(&m)) {
            return Some(p.to_owned());
        }
        if macos && m.is_dir() && has_extension(p, "app") {
            return Some(p.to_owned());
        }
    }
    if !windows {
        return None;
    }
    let (dir, name) = (p.parent()?, p.file_name()?.to_str()?.to_lowercase());
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_str().is_some_and(|n| n.to_lowercase() == name))
        .map(|e| e.path())
        .filter(|found| found.is_file())
}

/// Whether a file's permissions let it run (always true where there are no such bits).
fn executable(m: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        true
    }
}

/// [`locate`] for this system.
pub fn locate_here(p: &Path) -> Option<PathBuf> {
    locate(p, cfg!(windows), cfg!(target_os = "macos"))
}

/// Where `program` is on this system's PATH (see [`resolve`]), honouring the PATH Trellis
/// gives its children ([`crate::env_path`]).
pub fn find_on_path(program: &Path) -> Option<PathBuf> {
    let path_var = crate::env_path::path();
    let pathext = std::env::var_os("PATHEXT");
    resolve(program, path_var.as_deref(), pathext.as_deref(), cfg!(windows), &locate_here)
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
/// started through `open -a`, everything else as is. `open -a App <files>` hands the files to
/// the app as documents, even to a running one, but would take options meant for the app as its
/// own; so when any argument is an option (`--goto`, `-n`) they go after `--args`, to a new
/// instance (`-n`), since a running one never sees `--args` (editors that take options pass
/// them on to the window that is already open). With `wait`, `open -W` waits until the app
/// exits, so a caller that waits for the program (a merge tool) really waits.
pub(crate) fn command_line(resolved: &Path, args: &[OsString], macos: bool, wait: bool) -> (PathBuf, Vec<OsString>) {
    if macos && has_extension(resolved, "app") {
        let options = args.iter().any(|a| a.to_string_lossy().starts_with('-'));
        let mut all: Vec<OsString> = if wait { vec!["-W".into()] } else { Vec::new() };
        if options {
            all.push("-n".into());
        }
        all.extend([OsString::from("-a"), resolved.as_os_str().to_owned()]);
        if options {
            all.push("--args".into());
        }
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

/// How long a started program is watched for failing at once (a bad option, a missing file,
/// `open` not finding the app). One still running then is left to run.
pub const EARLY_EXIT: Duration = Duration::from_millis(1500);

/// Starts `l`, watching it for [`EARLY_EXIT`]: an exit with an error in that time is reported
/// with the first line it printed on stderr; after it, the program is left running, reaped on a
/// background thread. See `console_setup` for its input and output. `Error::NotFound` when the
/// program can't be found.
pub fn spawn(l: &Launch) -> Result<()> {
    let mut child = start(l, false)?;
    let stderr = child.stderr.take().map(drain_stderr);
    let deadline = Instant::now() + EARLY_EXIT;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                let said = stderr.and_then(|rx| rx.recv_timeout(Duration::from_millis(500)).ok()).unwrap_or_default();
                return Err(Error::Other(early_exit_message(status.code(), &said)));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break,
        }
    }
    // Reaped so it leaves no zombie on Unix; Trellis doesn't wait for the program.
    let _ = std::thread::Builder::new().name("trellis-tool-wait".into()).spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Starts `l` as `spawn` and `run_to_exit` describe; `wait` makes a macOS application bundle
/// run through `open -W`.
fn start(l: &Launch, wait: bool) -> Result<std::process::Child> {
    let macos = cfg!(target_os = "macos");
    let resolved = find_on_path(&l.program).ok_or_else(|| Error::NotFound(l.program.display().to_string()))?;
    let (program, args) = command_line(&resolved, &l.args, macos, wait);
    let mut cmd = Command::new(&program);
    cmd.args(&args).current_dir(native(&l.cwd));
    crate::env_path::apply(&mut cmd);
    let batch = cfg!(windows) && (has_extension(&resolved, "cmd") || has_extension(&resolved, "bat"));
    let setup = console_setup(l.own_console, batch);
    if setup.null_stdio {
        // stderr is kept, to say why a program that fails failed.
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(setup.creation_flags);
    }
    cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Error::NotFound(l.program.display().to_string()),
        _ => Error::Io(e),
    })
}

/// How a program Trellis waited for ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitReport {
    /// The exit code; None when it was stopped by a signal.
    pub code: Option<i32>,
    /// The first non-empty line it printed on stderr, made printable; "" when none.
    pub said: String,
}

impl ExitReport {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Starts `l` (with no input or output, as an editor; see `console_setup`) and waits until it
/// exits, however long that is. A macOS application bundle runs through `open -W`, so this
/// waits for the app too. `Error::NotFound` when the program can't be found.
pub fn run_to_exit(l: &Launch) -> Result<ExitReport> {
    let mut child = start(&Launch { own_console: false, ..l.clone() }, true)?;
    let stderr = child.stderr.take().map(drain_stderr);
    let status = child.wait()?;
    let text = stderr.and_then(|rx| rx.recv_timeout(Duration::from_secs(2)).ok()).unwrap_or_default();
    let said = text.lines().map(str::trim).find(|l| !l.is_empty()).map(crate::text::printable).unwrap_or_default();
    Ok(ExitReport { code: status.code(), said })
}

/// Reads a child's stderr to its end on a thread of its own (so a chatty program never blocks
/// on a full pipe), keeping the first few kilobytes.
fn drain_stderr(mut pipe: std::process::ChildStderr) -> std::sync::mpsc::Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new().name("trellis-tool-stderr".into()).spawn(move || {
        use std::io::Read;
        let (mut kept, mut buf) = (Vec::new(), [0u8; 4096]);
        while let Ok(n) = pipe.read(&mut buf) {
            if n == 0 {
                break;
            }
            if kept.len() < 4096 {
                kept.extend_from_slice(&buf[..n]);
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&kept).into_owned());
    });
    rx
}

/// What to say about a program that exited with `code` right after starting, having printed
/// `stderr`.
pub fn early_exit_message(code: Option<i32>, stderr: &str) -> String {
    let status = match code {
        Some(c) => format!("it exited at once with code {c}"),
        None => "it stopped at once".to_owned(),
    };
    match stderr.lines().map(str::trim).find(|l| !l.is_empty()) {
        Some(line) => format!("{status}: {}", crate::text::printable(line)),
        None => status,
    }
}

#[cfg(test)]
mod template_tests;
#[cfg(test)]
mod tests;
