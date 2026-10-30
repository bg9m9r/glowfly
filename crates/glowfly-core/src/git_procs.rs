//! Whether a git program may be working in a repository, for Delete lock file: a lock file is
//! only deleted when the answer is a sure No. Glowfly can't tell reliably which repository a
//! git process works in (`git -C`, `GIT_DIR`, a working folder elsewhere, a hook's child), so
//! any git process on the machine counts, except the two daemons that stay up without taking a
//! repository's locks (`git credential-cache--daemon`, `git fsmonitor--daemon`), and those only
//! when their command line can be read and says so. The lists are read in `git_procs/os.rs`
//! (through `terminal/procs.rs`): a Windows ToolHelp snapshot, Linux `/proc`, macOS libproc.
//! Anything that can't be read is `Unknown`, never No: the process list itself, a process's
//! name, `/proc` mounted to hide other users' processes, the Flatpak sandbox (which sees only
//! its own processes), and on Windows an open WSL session, whose git isn't a Windows process.
//! Reading takes milliseconds: worker threads only.

mod os;

/// What the process list says about git.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitProcesses {
    /// No git process is running.
    None,
    /// These git programs are running: `(pid, name)`, at most `SHOWN` of them.
    Running(Vec<(u32, String)>),
    /// It couldn't be told, for this reason.
    Unknown(Unknown),
}

/// Why `GitProcesses` couldn't be told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unknown {
    /// The system's process list couldn't be read (or this system has none Glowfly reads).
    NoList,
    /// A process's name couldn't be read while it still ran.
    Unreadable,
    /// `/proc` hides other users' processes (`hidepid`).
    Hidden,
    /// Glowfly runs in the Flatpak sandbox, which sees only its own processes.
    Sandbox,
    /// A WSL session is open on Windows: a git inside it isn't a Windows process.
    Wsl,
}

/// The most running git programs named.
const SHOWN: usize = 3;

/// The daemons that stay up for a long time without taking any repository's locks.
const DAEMONS: [&str; 2] = ["credential-cache--daemon", "fsmonitor--daemon"];

/// The git programs running now (see the module docs).
pub fn scan() -> GitProcesses {
    if crate::flatpak::in_flatpak() {
        return GitProcesses::Unknown(Unknown::Sandbox);
    }
    match os::list() {
        Ok(list) => judge(list, std::process::id(), os::args),
        Err(why) => GitProcesses::Unknown(why),
    }
}

/// Whether a process with `pid` runs now; None when that can't be told (no process list, or
/// the Flatpak sandbox, which sees only its own processes). Reads the whole list: worker
/// threads only.
pub fn running(pid: u32) -> Option<bool> {
    if crate::flatpak::in_flatpak() {
        return None;
    }
    os::list().ok().map(|list| list.iter().any(|(p, _)| *p == pid))
}

/// One process of a list: its pid and its executable's name, None when that couldn't be read.
pub(crate) type Proc = (u32, Option<String>);

/// The verdict on `list`, leaving out Glowfly itself (`own`) and the daemons whose command line
/// (`args`, read only for git processes) shows them. A git program found is reported even when
/// other names couldn't be read; otherwise an unreadable name or an open WSL session is Unknown.
pub(crate) fn judge(list: Vec<Proc>, own: u32, args: impl Fn(u32) -> Option<Vec<String>>) -> GitProcesses {
    let mut running = Vec::new();
    let mut unknown = None;
    for (pid, name) in list.into_iter().filter(|(pid, _)| *pid != own) {
        match name {
            Some(n) if is_git(&n) => {
                if !is_daemon(&n, args(pid).as_deref()) {
                    running.push((pid, n));
                }
            }
            Some(n) if is_wsl(&n) => unknown = unknown.or(Some(Unknown::Wsl)),
            Some(_) => {}
            None => unknown = Some(Unknown::Unreadable),
        }
    }
    if !running.is_empty() {
        running.sort();
        running.truncate(SHOWN);
        return GitProcesses::Running(running);
    }
    unknown.map_or(GitProcesses::None, GitProcesses::Unknown)
}

/// The name without a Windows `.exe`, any case.
fn stem(name: &str) -> &str {
    let n = name.len();
    if n > 4 && name.is_char_boundary(n - 4) && name[n - 4..].eq_ignore_ascii_case(".exe") { &name[..n - 4] } else { name }
}

/// The part of a name after `git-`, any case.
fn after_git_dash(name: &str) -> Option<&str> {
    let s = stem(name);
    (s.len() > 4 && s.is_char_boundary(4) && s[..4].eq_ignore_ascii_case("git-")).then(|| &s[4..])
}

/// git itself and its helpers (`git-remote-https`, `git-upload-pack`, `git-lfs`,
/// `git-credential-manager`), in any case: any of them may hold or be about to take a lock.
pub(crate) fn is_git(name: &str) -> bool {
    stem(name).eq_ignore_ascii_case("git") || after_git_dash(name).is_some()
}

/// A long-running daemon that takes no repository's locks: by the program's own name
/// (`git-credential-cache--daemon`), or by its command line (`args`, the program first) whose
/// subcommand, after git's own options, is one of `DAEMONS`. A command line that couldn't be
/// read is no daemon.
pub(crate) fn is_daemon(name: &str, args: Option<&[String]>) -> bool {
    if after_git_dash(name).is_some_and(|d| DAEMONS.iter().any(|x| d.eq_ignore_ascii_case(x))) {
        return true;
    }
    let Some([_, rest @ ..]) = args else { return false };
    let mut i = 0;
    while let Some(a) = rest.get(i) {
        match a.as_str() {
            "-c" | "-C" => i += 2,
            a if a.starts_with('-') => i += 1,
            a => return DAEMONS.contains(&a),
        }
    }
    false
}

/// A Windows command line split into its arguments: unquoted spaces part them, double quotes
/// group, and a backslash before a quote escapes it (enough to find git's subcommand).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn split_command_line(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => (quoted, any) = (!quoted, true),
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The Windows programs of an open WSL session.
fn is_wsl(name: &str) -> bool {
    cfg!(windows) && ["wsl", "wslhost"].iter().any(|w| stem(name).eq_ignore_ascii_case(w))
}

/// Whether a `/proc/mounts` text mounts `/proc` with `hidepid` set to anything but 0 or off.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn proc_hidden(mounts: &str) -> bool {
    mounts.lines().any(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        f.len() >= 4
            && f[1] == "/proc"
            && f[2] == "proc"
            && f[3].split(',').filter_map(|o| o.strip_prefix("hidepid=")).any(|v| v != "0" && v != "off")
    })
}

#[cfg(test)]
#[path = "git_procs_tests.rs"]
mod tests;
