//! Whether a terminal's shell is running a program, so closing it can ask first. Windows reads
//! the shell's child processes; Unix compares the terminal's foreground process group with the
//! shell. Reading it can take milliseconds, so callers ask off the UI thread.

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
    match (p.pid, process_table()) {
        (Some(pid), Some(table)) => tree_activity(&table, pid),
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
    foreground_activity(leader, p.pid, process_name)
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

/// Console hosts that Windows attaches to a console program; they are never the user's.
fn console_host(exe: &str) -> bool {
    exe.eq_ignore_ascii_case("conhost.exe") || exe.eq_ignore_ascii_case("openconsole.exe")
}

/// The activity of `shell` in a process snapshot: no children is Idle, and a single child with
/// the same executable name is a launcher (Git Bash's `bash.exe` starting the real one) that is
/// followed. Any other child is the running program.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn tree_activity(procs: &[ProcEntry], shell: u32) -> ShellActivity {
    let mut at = shell;
    for _ in 0..LAUNCHER_DEPTH {
        let exe = procs.iter().find(|p| p.pid == at).map_or("", |p| p.exe.as_str());
        let kids: Vec<&ProcEntry> = procs.iter().filter(|p| p.parent == at && p.pid != at && !console_host(&p.exe)).collect();
        match kids.as_slice() {
            [] => return ShellActivity::Idle,
            [kid] if kid.exe.eq_ignore_ascii_case(exe) => at = kid.pid,
            [first, ..] => {
                let stem =
                    std::path::Path::new(&first.exe).file_stem().map_or_else(|| first.exe.clone(), |s| s.to_string_lossy().into_owned());
                return ShellActivity::Running { program: Some(stem) };
            }
        }
    }
    ShellActivity::Idle
}

/// The activity from the terminal's foreground process group `leader`: the shell's own group
/// is Idle, any other is Running, named by `name`.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn foreground_activity(leader: Option<i32>, shell: Option<u32>, name: impl Fn(i32) -> Option<String>) -> ShellActivity {
    match (leader, shell) {
        (Some(l), Some(s)) if l > 0 => {
            if l.unsigned_abs() == s {
                ShellActivity::Idle
            } else {
                ShellActivity::Running { program: name(l) }
            }
        }
        _ => ShellActivity::Unknown,
    }
}

/// Every process on the system, from a ToolHelp snapshot.
#[cfg(windows)]
fn process_table() -> Option<Vec<ProcEntry>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    // SAFETY: the call takes no pointers; the returned handle is closed below.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut out = Vec::new();
    // SAFETY: `snap` is an open snapshot handle and `entry` a writable PROCESSENTRY32W whose
    // dwSize is set, as both calls require.
    let mut more = unsafe { Process32FirstW(snap, &mut entry) }.is_ok();
    while more {
        let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
        out.push(ProcEntry {
            pid: entry.th32ProcessID,
            parent: entry.th32ParentProcessID,
            exe: String::from_utf16_lossy(&entry.szExeFile[..len]),
        });
        // SAFETY: as above.
        more = unsafe { Process32NextW(snap, &mut entry) }.is_ok();
    }
    // SAFETY: `snap` is open and not used after this.
    let _ = unsafe { CloseHandle(snap) };
    Some(out)
}

/// A process's command name.
#[cfg(target_os = "linux")]
fn process_name(pid: i32) -> Option<String> {
    let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = name.trim_end();
    (!name.is_empty()).then(|| name.to_string())
}

/// macOS and the BSDs have no file to read it from.
#[cfg(all(unix, not(target_os = "linux")))]
fn process_name(_pid: i32) -> Option<String> {
    None
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
