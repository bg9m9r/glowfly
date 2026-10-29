//! Starting the update helper, and the helper starting Glowfly again, so that neither ends with
//! the process that started it: on Windows in a new process group, without a console and out
//! of the starter's job when that is allowed; on Unix in a new session, so a closing terminal
//! doesn't take it along. Also whether a copied helper can run where it was put ([`try_run`]).

use std::ffi::OsString;
use std::path::Path;
use std::process::{Child, Command, Stdio};

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

/// Whether `program` (a copy of Glowfly) starts where it is, by running it with `--version`:
/// on Linux and macOS a folder mounted noexec refuses with "permission denied". Windows has no
/// such mounts, so nothing is started there.
pub fn try_run(program: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        Command::new(program).arg(super::VERSION_FLAG).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(drop)
    }
    #[cfg(not(unix))]
    {
        let _ = program;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_flags() {
        assert_eq!(creation_flags(true), 0x0100_0208);
        assert_eq!(creation_flags(false), 0x0000_0208);
    }
}
