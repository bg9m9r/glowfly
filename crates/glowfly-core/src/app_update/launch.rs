//! Starting the update helper, and the helper starting Glowfly again, so that neither ends with
//! the process that started it: on Windows in a new process group, without a console and out
//! of the starter's job when that is allowed; on Unix in a new session, so a closing terminal
//! doesn't take it along. Also whether a copied helper can run where it was put ([`try_run`]).

use std::ffi::OsString;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

/// The Windows creation flags of a detached start; `breakaway` also leaves the starter's job.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn creation_flags(breakaway: bool) -> u32 {
    let base = CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS;
    if breakaway { base | CREATE_BREAKAWAY_FROM_JOB } else { base }
}

/// Starts `cmd` detached. A job that forbids breaking away refuses with "access denied", so
/// on Windows that is tried again without it.
fn spawn_detached(cmd: &mut Command) -> std::io::Result<Child> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(creation_flags(true));
        match cmd.spawn() {
            Err(e) if e.raw_os_error() == Some(5) => {
                cmd.creation_flags(creation_flags(false));
                cmd.spawn()
            }
            r => r,
        }
    }
    #[cfg(unix)]
    {
        crate::ssh_version::detach_from_terminal(cmd);
        cmd.spawn()
    }
    #[cfg(not(any(windows, unix)))]
    {
        cmd.spawn()
    }
}

/// Starts the helper `program` on the plan at `plan`, in `cwd`, with its stdin piped (the go
/// signal, then Glowfly's exit) and no other output.
pub fn start_helper(program: &Path, plan: &Path, cwd: &Path) -> std::io::Result<Child> {
    let mut cmd = Command::new(program);
    cmd.arg(super::FINISH_FLAG).arg(plan).current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    spawn_detached(&mut cmd)
}

/// Starts `program` with `args` (in `cwd` when given), detached and without stdio.
pub fn start_detached(program: &Path, args: &[OsString], cwd: Option<&Path>) -> std::io::Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    // Not waited for: it runs on after the helper exits.
    spawn_detached(&mut cmd).map(drop)
}

/// How long a tried helper may take to print its version.
#[cfg_attr(not(unix), allow(dead_code))]
pub const TRY_TIMEOUT: Duration = Duration::from_secs(5);
/// The most of a tried helper's output that is read.
#[cfg_attr(not(unix), allow(dead_code))]
const TRY_OUTPUT: u64 = 4096;

/// Whether `program` (a copy of Glowfly) runs where it is: started with `--version`, it must
/// print this build's `glowfly <version>` and exit successfully within [`TRY_TIMEOUT`]. On Linux
/// and macOS a folder mounted noexec refuses to start it ("permission denied"). Windows has no
/// such mounts, so nothing is started there.
pub fn try_run(program: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut cmd = Command::new(program);
        cmd.arg(super::VERSION_FLAG);
        probe(cmd, &format!("glowfly {}", super::CURRENT_VERSION), TRY_TIMEOUT)
    }
    #[cfg(not(unix))]
    {
        let _ = program;
        Ok(())
    }
}

/// Runs `cmd` (no input, its errors dropped) and checks that it exits successfully within
/// `timeout` with `expected` as its whole output (surrounding whitespace aside). One that takes
/// longer is killed and counts as not running.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn probe(mut cmd: Command, expected: &str, timeout: Duration) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind, Read};
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let out = child.stdout.take();
    // Read on a thread: a program that keeps its output open can't hold the wait up.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(out) = out {
            let _ = out.take(TRY_OUTPUT).read_to_end(&mut text);
        }
        let _ = tx.send(text);
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::new(ErrorKind::TimedOut, format!("it didn't finish within {} s", timeout.as_secs_f32())));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if !status.success() {
        return Err(Error::other(format!("it exited with {status}")));
    }
    let text = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_default();
    let text = String::from_utf8_lossy(&text);
    if text.trim() != expected {
        return Err(Error::other(format!("it printed {:?}, not {expected:?}", text.trim())));
    }
    Ok(())
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
