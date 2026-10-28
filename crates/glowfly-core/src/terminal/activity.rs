//! Whether a terminal's shell is running a program, so closing it, closing its repository's tab
//! or quitting can ask first. Windows reads the shell's child processes; Unix compares the
//! terminal's foreground process group with the shell, then looks for the shell's children
//! (background jobs). The systems' process lists are read in procs.rs. Reading it can take
//! milliseconds, so callers ask off the UI thread.

/// What a shell is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellActivity {
    /// Waiting at its prompt.
    Idle,
    /// Running a program, by name when the system tells it.
    Running { program: Option<String> },
    /// It couldn't be read.
    Unknown,
}

/// Reads a session's shell activity from any thread. It never keeps the terminal open. The
/// default probes no process and is always Unknown.
#[derive(Clone, Default)]
pub struct ShellProbe {
    pub(super) pid: Option<u32>,
    #[cfg(unix)]
    pub(super) master: std::sync::Weak<std::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
}

impl ShellProbe {
    /// What the shell is doing right now.
    pub fn activity(&self) -> ShellActivity {
        os_activity(self)
    }
}

#[cfg(windows)]
fn os_activity(p: &ShellProbe) -> ShellActivity {
    match (p.pid, super::procs::process_table()) {
        (Some(pid), Some(table)) => tree_activity(&table, pid, super::procs::image_path),
        _ => ShellActivity::Unknown,
    }
}

#[cfg(unix)]
fn os_activity(p: &ShellProbe) -> ShellActivity {
    // A closed terminal (or a poisoned lock) reads as no foreground group: Unknown.
    let leader = p.master.upgrade().and_then(|master| {
        let pty = master.lock().ok()?;
        pty.process_group_leader()
    });
    foreground_activity(leader, p.pid, super::procs::process_name, super::procs::children)
}

#[cfg(not(any(windows, unix)))]
fn os_activity(_p: &ShellProbe) -> ShellActivity {
    ShellActivity::Unknown
}

/// One process of a system snapshot.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct ProcEntry {
    pub pid: u32,
    pub parent: u32,
    /// The executable's file name, e.g. `git.exe`.
    pub exe: String,
}

/// How deep a chain of same-named launchers is followed.
const LAUNCHER_DEPTH: usize = 4;

/// Processes Windows or WSL attach to a shell that are never the user's: console hosts, and the
/// host WSL keeps beside `wsl.exe`.
fn helper(exe: &str) -> bool {
    ["conhost.exe", "openconsole.exe", "wslhost.exe"].iter().any(|h| exe.eq_ignore_ascii_case(h))
}

/// The activity of `shell` in a process snapshot: no children is Idle, and a single child with
/// the same executable name is a launcher (Git Bash's `bin\bash.exe` starting `usr\bin\bash.exe`,
/// `wsl.exe` starting WSL's own) that is followed, unless `image` says both run the same file
/// (PowerShell's Start-Job starting another pwsh.exe: a background job). Any other child is the
/// running program, whether it runs in front or in the background. A `wsl.exe` with no
/// child of its own is Unknown: the programs inside the distribution aren't Windows processes.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn tree_activity(procs: &[ProcEntry], shell: u32, image: impl Fn(u32) -> Option<String>) -> ShellActivity {
    let mut at = shell;
    for _ in 0..LAUNCHER_DEPTH {
        let exe = procs.iter().find(|p| p.pid == at).map_or("", |p| p.exe.as_str());
        let kids: Vec<&ProcEntry> = procs.iter().filter(|p| p.parent == at && p.pid != at && !helper(&p.exe)).collect();
        match kids.as_slice() {
            [] if exe.eq_ignore_ascii_case("wsl.exe") => return ShellActivity::Unknown,
            [] => return ShellActivity::Idle,
            [kid] if kid.exe.eq_ignore_ascii_case(exe) && !same_file(image(at), image(kid.pid)) => at = kid.pid,
            [first, ..] => {
                let stem =
                    std::path::Path::new(&first.exe).file_stem().map_or_else(|| first.exe.clone(), |s| s.to_string_lossy().into_owned());
                return ShellActivity::Running { program: Some(stem) };
            }
        }
    }
    ShellActivity::Idle
}

/// Both image paths are known and name one file (Windows paths, compared ignoring case).
fn same_file(a: Option<String>, b: Option<String>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a.eq_ignore_ascii_case(&b))
}

/// The activity from the terminal's foreground process group `leader`: another group is the
/// running program, named by `name`; the shell's own group is Idle unless the shell has
/// `children` (background jobs, `cmd &`), when the first of them is the running program.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn foreground_activity(
    leader: Option<i32>,
    shell: Option<u32>,
    name: impl Fn(i32) -> Option<String>,
    children: impl Fn(u32) -> Vec<i32>,
) -> ShellActivity {
    match (leader, shell) {
        (Some(l), Some(s)) if l > 0 => {
            if l.unsigned_abs() != s {
                return ShellActivity::Running { program: name(l) };
            }
            match children(s).first() {
                Some(&kid) => ShellActivity::Running { program: name(kid) },
                None => ShellActivity::Idle,
            }
        }
        _ => ShellActivity::Unknown,
    }
}

/// What `/proc/<pid>/stat` tells about a process.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stat {
    /// `R`, `S`, `Z` (ended, not yet reaped) and so on.
    pub state: char,
    pub ppid: i32,
    pub session: i32,
}

/// Reads a `/proc/<pid>/stat` line: `pid (comm) state ppid pgrp session …`. The command name
/// may hold spaces and parentheses, so the fields start after the last `)`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_stat(line: &str) -> Option<Stat> {
    let rest = &line[line.rfind(')')? + 1..];
    let mut fields = rest.split_whitespace();
    let state = fields.next()?.chars().next()?;
    let ppid = fields.next()?.parse().ok()?;
    let _pgrp = fields.next()?;
    let session = fields.next()?.parse().ok()?;
    Some(Stat { state, ppid, session })
}

/// The pids among `all` that are children of `shell` in its `session` and still run: a
/// background job, not a helper that left the terminal's session or one that ended.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn children_in_session(all: &[(i32, Stat)], shell: i32, session: i32) -> Vec<i32> {
    all.iter().filter(|(_, s)| s.ppid == shell && s.session == session && s.state != 'Z').map(|(pid, _)| *pid).collect()
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
