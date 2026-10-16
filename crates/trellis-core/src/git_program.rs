//! Which git program every git child runs: the one chosen in Preferences > Git, else `git`
//! looked up on the PATH children get ([`crate::env_path`]). Unlike the askpass environment in
//! [`crate::gitcli`], this can change while Trellis runs; a job already running keeps the git it
//! started with. The chosen program's folder is never added to any PATH: the built-in terminal
//! and external tools keep finding whatever git the user's PATH finds.
//!
//! Also how a program is checked before it is used ([`probe`]) and how the gits installed on
//! this machine are found ([`discover`]).

use crate::external_tools;
use crate::git_version::Version;
use crate::gitcli;
use crate::native_path::native;
use crate::opens::Os;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;

static CHOSEN: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Makes every later git child run `program` (an absolute path), or `git` from PATH for None.
pub fn set(program: Option<&Path>) {
    *CHOSEN.write().unwrap_or_else(|e| e.into_inner()) = program.map(Path::to_path_buf);
}

/// The program chosen in Preferences > Git, if any.
pub fn chosen() -> Option<PathBuf> {
    CHOSEN.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// What git children are started as: the chosen program in this system's spelling, else `git`.
pub fn program() -> OsString {
    chosen().map(|p| native(&p).into_os_string()).unwrap_or_else(|| "git".into())
}

/// The program a saved `git_program` setting names: trimmed, without one pair of quotes, and
/// only when it is an absolute path. Empty or relative values mean git from PATH. Pure.
pub fn from_setting(value: Option<&str>) -> Option<PathBuf> {
    external_tools::program(value?).filter(|p| p.is_absolute())
}

/// How long `git --version` may take before a program is refused.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Why a program can't be used as git.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing is at that path.
    Missing,
    /// It is a folder.
    Folder,
    /// It opens a window of its own (`git-bash`, `gitk`, ...) rather than being git.
    Launcher,
    /// Its path isn't valid Unicode, so the setting can't hold it.
    NotUnicode,
    /// The system wouldn't start it (the OS's reason).
    CantRun(String),
    /// It ran and succeeded but didn't print a git version (its first line).
    NotGit(String),
    /// It ran and failed: its exit code (None when stopped by a signal) and first line.
    Failed { code: Option<i32>, line: String },
    /// It didn't answer within [`PROBE_TIMEOUT`].
    TimedOut,
}

impl Refusal {
    /// Why, as the end of "Can't use <path>: ...".
    pub fn text(&self) -> String {
        match self {
            Refusal::Missing => "there is no file there".to_owned(),
            Refusal::Folder => "it's a folder, not a program".to_owned(),
            Refusal::Launcher => "it starts a window, not git; choose cmd/git.exe (Windows) or bin/git".to_owned(),
            Refusal::NotUnicode => "Trellis can only save paths that are valid Unicode".to_owned(),
            Refusal::CantRun(why) => format!("it can't be started ({why})"),
            Refusal::NotGit(line) if line.is_empty() => "it isn't a working git: it printed no git version".to_owned(),
            Refusal::NotGit(line) => format!("it isn't a working git: it printed \"{line}\""),
            Refusal::Failed { code, line } => {
                let how = match code {
                    Some(c) => format!("it exited with code {c}"),
                    None => "it was stopped".to_owned(),
                };
                if line.is_empty() { format!("it isn't a working git: {how}") } else { format!("it isn't a working git: {how} ({line})") }
            }
            Refusal::TimedOut => format!("it didn't answer within {} seconds", PROBE_TIMEOUT.as_secs()),
        }
    }
}

/// Programs that come with git but open a window instead of acting as git.
const LAUNCHERS: [&str; 4] = ["git-bash", "git-cmd", "git-gui", "gitk"];

/// Whether `file_name` is one of the [`LAUNCHERS`]: on Windows in any case and with an optional
/// `.exe`; elsewhere spelled exactly. Pure.
pub fn is_launcher(file_name: &OsStr, windows: bool) -> bool {
    let Some(name) = file_name.to_str() else { return false };
    if !windows {
        return LAUNCHERS.contains(&name);
    }
    let lower = name.to_lowercase();
    LAUNCHERS.contains(&lower.strip_suffix(".exe").unwrap_or(&lower))
}

/// The checks made before anything runs: the file exists, isn't a folder or a launcher, and (on
/// Unix) may be run.
pub fn precheck(p: &Path) -> Result<(), Refusal> {
    let meta = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Refusal::Missing),
        Err(e) => return Err(Refusal::CantRun(e.to_string())),
    };
    if meta.is_dir() {
        return Err(Refusal::Folder);
    }
    if p.file_name().is_some_and(|n| is_launcher(n, cfg!(windows))) {
        return Err(Refusal::Launcher);
    }
    if cfg!(unix) && external_tools::locate_here(p).is_none() {
        return Err(Refusal::CantRun("no permission to run it".to_owned()));
    }
    Ok(())
}

/// The first non-empty line of `text`, made printable and cut to 80 characters.
fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    crate::text::printable(line).chars().take(80).collect()
}

/// Runs `p --version` in `cwd` (with no prompts, no console window, and killed after
/// [`PROBE_TIMEOUT`]) and returns the git version it prints, or why it can't be used.
pub fn probe(p: &Path, cwd: &Path) -> Result<Version, Refusal> {
    precheck(p)?;
    let out = gitcli::output_with(native(p).as_os_str(), cwd, &gitcli::args(&["--version"]), "", gitcli::NO_PROMPTS, Some(PROBE_TIMEOUT));
    if out.timed_out {
        return Err(Refusal::TimedOut);
    }
    if out.code.is_none()
        && let Some(why) = out.stderr.strip_prefix(gitcli::SPAWN_FAILED)
    {
        return Err(Refusal::CantRun(first_line(why)));
    }
    if out.ok {
        return gitcli::parse_version(&out.stdout).ok_or_else(|| Refusal::NotGit(first_line(&out.stdout)));
    }
    let line = if out.stderr.trim().is_empty() { first_line(&out.stdout) } else { first_line(&out.stderr) };
    Err(Refusal::Failed { code: out.code, line })
}

/// A git found on this machine, and what checking it said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// As found (never resolved, so a Homebrew link keeps working after an upgrade).
    pub path: PathBuf,
    /// The git a bare `git` runs: the first on the PATH children get.
    pub first_on_path: bool,
    /// Its version, or why it can't be used ([`Refusal::text`]).
    pub check: Result<Version, String>,
}

/// Apple's `/usr/bin/git`: a shim that asks to install the developer tools when they are missing.
const APPLE_SHIM: &str = "/usr/bin/git";
const APPLE_TOOLS_GITS: [&str; 2] =
    ["/Library/Developer/CommandLineTools/usr/bin/git", "/Applications/Xcode.app/Contents/Developer/usr/bin/git"];

/// Where git is usually installed on `os`, from the environment (`env`) and the home folder.
/// Unset, empty or relative variables are skipped. Pure.
pub fn known_locations(os: Os, env: &dyn Fn(&str) -> Option<OsString>, home: Option<&Path>) -> Vec<PathBuf> {
    let var = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from).filter(|p| p.is_absolute());
    let cmd_git = |base: PathBuf| base.join("cmd").join("git.exe");
    let fixed = |list: &[&str]| list.iter().map(PathBuf::from).collect::<Vec<_>>();
    let in_home = |rel: &[&str]| home.map(|h| rel.iter().fold(h.to_path_buf(), |p, part| p.join(part)));
    let mut out = Vec::new();
    match os {
        Os::Windows => {
            for k in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
                out.extend(var(k).map(|v| cmd_git(v.join("Git"))));
            }
            out.extend(var("LOCALAPPDATA").map(|v| cmd_git(v.join("Programs").join("Git"))));
            let scoop = var("SCOOP").or_else(|| home.map(|h| h.join("scoop")));
            out.extend(scoop.map(|s| cmd_git(s.join("apps").join("git").join("current"))));
        }
        Os::MacOs => {
            out.extend(fixed(&["/opt/homebrew/bin/git", "/usr/local/bin/git", "/opt/local/bin/git"]));
            out.extend(fixed(&APPLE_TOOLS_GITS));
            out.push(PathBuf::from(APPLE_SHIM));
            out.extend(in_home(&[".nix-profile", "bin", "git"]));
        }
        Os::Linux => {
            out.extend(fixed(&["/usr/bin/git", "/usr/local/bin/git", "/bin/git", "/home/linuxbrew/.linuxbrew/bin/git"]));
            out.extend(in_home(&[".linuxbrew", "bin", "git"]));
            out.extend(in_home(&[".local", "bin", "git"]));
            out.extend(in_home(&[".nix-profile", "bin", "git"]));
            out.push(PathBuf::from("/run/current-system/sw/bin/git"));
        }
    }
    out
}

/// Whether running Apple's `/usr/bin/git` is safe: the developer tools it forwards to are
/// installed (otherwise it opens their install dialog). Pure.
pub fn apple_shim_usable(exists: &dyn Fn(&Path) -> bool) -> bool {
    APPLE_TOOLS_GITS.iter().any(|p| exists(Path::new(p)))
}

/// The gits to list: `on_path` first (the first of them flagged as the PATH git), then those of
/// `known` that `locate` finds. An entry `same` as one already listed is skipped, keeping the
/// spelling found first.
pub fn gather(
    on_path: Vec<PathBuf>,
    known: Vec<PathBuf>,
    locate: &dyn Fn(&Path) -> Option<PathBuf>,
    same: &dyn Fn(&Path, &Path) -> bool,
) -> Vec<(PathBuf, bool)> {
    let found = on_path.into_iter().map(|p| (p, true)).chain(known.iter().filter_map(|p| locate(p)).map(|p| (p, false)));
    let mut out: Vec<(PathBuf, bool)> = Vec::new();
    for (p, from_path) in found {
        if out.iter().any(|(q, _)| same(q, &p)) {
            continue;
        }
        let first = from_path && out.is_empty();
        out.push((p, first));
    }
    out
}

/// Every `git` on `path_var`, in order (one per folder, with the extension rules of
/// [`external_tools::resolve`]). Empty and relative entries are skipped: they would mean a
/// folder relative to wherever the search runs.
pub fn all_on_path(
    path_var: Option<&OsStr>,
    pathext: Option<&OsStr>,
    windows: bool,
    find: &dyn Fn(&Path) -> Option<PathBuf>,
) -> Vec<PathBuf> {
    let Some(path_var) = path_var else { return Vec::new() };
    let names = external_tools::candidate_names(Path::new("git"), pathext, windows);
    std::env::split_paths(path_var)
        .filter(|dir| dir.is_absolute())
        .filter_map(|dir| names.iter().find_map(|n| find(&dir.join(n))))
        .collect()
}

/// The gits installed on this machine, each checked with [`probe`] in parallel (asked in `cwd`,
/// a folder outside any repository). On macOS, `/usr/bin/git` is left out unless the developer
/// tools are installed, so checking it can't open their install dialog.
pub fn discover(cwd: &Path) -> Vec<Candidate> {
    let pathext = std::env::var_os("PATHEXT");
    let find = |p: &Path| external_tools::locate_here(p).map(as_listed);
    let on_path = all_on_path(crate::env_path::path().as_deref(), pathext.as_deref(), cfg!(windows), &find);
    let known = known_locations(Os::current(), &|k| std::env::var_os(k), crate::settings::home_dir().as_deref());
    let mut found = gather(on_path, known, &external_tools::locate_here, &|a, b| crate::paths::resolve(a) == crate::paths::resolve(b));
    if cfg!(target_os = "macos") && !apple_shim_usable(&|p| p.is_file()) {
        found.retain(|(p, _)| p != Path::new(APPLE_SHIM));
    }
    std::thread::scope(|s| {
        let checks: Vec<_> = found
            .into_iter()
            .map(|(path, first_on_path)| {
                s.spawn(move || {
                    let check = probe(&path, cwd).map_err(|r| r.text());
                    Candidate { path, first_on_path, check }
                })
            })
            .collect();
        checks.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// `p` with its file name spelled as its folder lists it: Windows finds `git.exe` when asked for
/// `git.EXE` (PATHEXT's spelling), and the list should show the real name.
fn as_listed(p: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return p;
    }
    let (Some(dir), Some(name)) = (p.parent(), p.file_name().and_then(OsStr::to_str)) else { return p };
    let name = name.to_lowercase();
    let listed = std::fs::read_dir(dir)
        .ok()
        .and_then(|entries| entries.flatten().map(|e| e.file_name()).find(|n| n.to_str().is_some_and(|n| n.to_lowercase() == name)));
    match listed {
        Some(n) => dir.join(n),
        None => p,
    }
}

#[cfg(test)]
#[path = "git_program_tests.rs"]
mod tests;
