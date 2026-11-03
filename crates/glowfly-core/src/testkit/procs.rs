//! Watching a shell a test starts through git (an alias, a remote program), so a test waits for
//! what it checks rather than sleeping for a fixed time: the shell writes its process id to a
//! file, the test waits for that file to know it started, and after stopping it, waits for the
//! process to be gone.

use std::path::Path;
use std::time::{Duration, Instant};

/// How often the waits here look again.
const POLL: Duration = Duration::from_millis(20);

/// `p` as a path git's shell can name, in single quotes.
fn quoted(p: &Path) -> String {
    format!("'{}'", p.display().to_string().replace('\\', "/"))
}

/// A shell snippet writing the id of the shell running it to `file`, as the OS lists it (Git for
/// Windows' shell numbers its processes itself: `/proc/$$/winpid` has the Windows id). The file
/// appears whole: it is written beside and moved in place.
pub fn write_pid(file: &Path) -> String {
    let part = file.with_extension("part");
    let (to, part) = (quoted(file), quoted(&part));
    format!("(cat /proc/$$/winpid 2>/dev/null || echo $$) > {part} && mv {part} {to}")
}

/// The process id `write_pid` wrote to `file`; None while it isn't there yet.
pub fn pid_in(file: &Path) -> Option<u32> {
    std::fs::read_to_string(file).ok()?.trim().parse().ok()
}

/// Waits until `cond` holds, up to `within`; whether it held.
pub fn wait_for(within: Duration, cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + within;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(POLL);
    }
}

/// Waits until the process `pid` is gone or `acted` holds (what the process does if it lives
/// on, such as writing a marker file), up to `within`.
pub fn wait_gone_or(pid: u32, within: Duration, acted: impl Fn() -> bool) {
    wait_for(within, || acted() || crate::git_procs::running(pid) != Some(true));
}
