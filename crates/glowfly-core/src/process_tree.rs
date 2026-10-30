//! Stopping a git child together with everything it started (git-remote-https, ssh, a hook's
//! shell), for [`crate::gitcli::output_until`].
//!
//! - Unix: the child starts in its own session and process group (`setsid`, as every git child
//!   does so ssh asks the askpass helper: [`crate::ssh_version::prepare`]). A stop sends
//!   SIGTERM to the group, so git's own signal handlers remove its lock files, and SIGKILL to
//!   the group after [`STOP_GRACE`] if git or anything else in the group is still running.
//!   SIGKILL goes either before git is reaped or while the group still has members, so a
//!   recycled process id is never signalled.
//! - Windows: the child is put in a job object right after it starts, and a stop terminates
//!   the job (there is no graceful signal for a windowless child), then waits up to
//!   [`STOP_GRACE`] for every process in it to be gone, so files they held (a clone's folder)
//!   can be removed. If the assignment fails the child alone is killed. The job is not killed
//!   when Glowfly exits.

use std::process::{Child, Command};
use std::time::Duration;

/// How long git gets to exit after SIGTERM before the group is killed (on Windows, how long a
/// terminated job's processes get to be gone).
pub(crate) const STOP_GRACE: Duration = Duration::from_secs(3);

/// Prepares `cmd` so its whole process tree can be stopped: a new session on Unix.
#[cfg(unix)]
pub(crate) fn prepare(cmd: &mut Command) {
    crate::ssh_version::detach_from_terminal(cmd);
}

/// Prepares `cmd` so its whole process tree can be stopped: nothing to do before spawning on
/// Windows (the job object is attached afterwards).
#[cfg(not(unix))]
pub(crate) fn prepare(_cmd: &mut Command) {}

/// A started child's process tree, for [`Tree::terminate`].
pub(crate) struct Tree {
    #[cfg(windows)]
    job: Option<windows::Win32::Foundation::HANDLE>,
    #[cfg(unix)]
    pgid: libc::pid_t,
}

/// The tree of `child`, which was started from a command given to [`prepare`].
#[cfg(unix)]
pub(crate) fn attach(child: &Child) -> Tree {
    // setsid made the child a process-group leader, so its group id is its pid.
    Tree { pgid: child.id() as libc::pid_t }
}

/// The tree of `child`: a new job object holding it, or none when that fails.
#[cfg(windows)]
pub(crate) fn attach(child: &Child) -> Tree {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW};
    use windows::core::PCWSTR;
    // SAFETY: an unnamed job object with default security; the handle is closed on drop.
    let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.ok();
    let job = job.filter(|&job| {
        // SAFETY: both handles are valid: the job was just created and `child` is still running
        // or a zombie we haven't waited for, so its process handle is open.
        let assigned = unsafe { AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())) }.is_ok();
        if !assigned {
            // SAFETY: the job handle is ours and not used again.
            let _ = unsafe { CloseHandle(job) };
        }
        assigned
    });
    Tree { job }
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn attach(_child: &Child) -> Tree {
    Tree {}
}

impl Tree {
    /// Stops `child` and the processes it started, then waits for `child`. Returns whether it
    /// took a hard kill.
    #[cfg(unix)]
    pub(crate) fn terminate(&self, child: &mut Child) -> bool {
        // SAFETY: kill only sends a signal; ESRCH (the group is gone) is fine.
        let _ = unsafe { libc::kill(-self.pgid, libc::SIGTERM) };
        let deadline = std::time::Instant::now() + STOP_GRACE;
        let mut exited = false;
        while std::time::Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => {
                    exited = true;
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => break,
            }
        }
        if exited {
            // git is gone; its remote helper or ssh may still be ending. A group with members
            // keeps its id, so it can't belong to anyone else while `kill(.., 0)` finds it.
            // SAFETY: signal 0 only checks; SIGKILL goes to a group that still exists.
            while unsafe { libc::kill(-self.pgid, 0) } == 0 {
                if std::time::Instant::now() >= deadline {
                    let _ = unsafe { libc::kill(-self.pgid, libc::SIGKILL) };
                    return true;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            return false;
        }
        // git hasn't been reaped, so its pid (the group's id) can't have been reused.
        // SAFETY: as above.
        let _ = unsafe { libc::kill(-self.pgid, libc::SIGKILL) };
        let _ = child.wait();
        true
    }

    /// Stops `child` and the processes it started, then waits for `child`. Always a hard kill.
    #[cfg(windows)]
    pub(crate) fn terminate(&self, child: &mut Child) -> bool {
        use windows::Win32::System::JobObjects::TerminateJobObject;
        match self.job {
            // SAFETY: the job handle is open until drop.
            Some(job) => {
                if unsafe { TerminateJobObject(job, 1) }.is_err() {
                    let _ = child.kill();
                }
            }
            None => {
                let _ = child.kill();
            }
        }
        let _ = child.wait();
        if let Some(job) = self.job {
            // Termination is asynchronous: wait until the helpers and ssh are gone too.
            let deadline = std::time::Instant::now() + STOP_GRACE;
            while active_processes(job).is_some_and(|n| n > 0) && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        true
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn terminate(&self, child: &mut Child) -> bool {
        let _ = child.kill();
        let _ = child.wait();
        true
    }
}

/// How many processes in `job` are still running; None when that can't be read.
#[cfg(windows)]
fn active_processes(job: windows::Win32::Foundation::HANDLE) -> Option<u32> {
    use windows::Win32::System::JobObjects::{
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation, QueryInformationJobObject,
    };
    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    let size = std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32;
    // SAFETY: the job handle is open until drop, and `info` is a writable buffer of `size` bytes.
    unsafe { QueryInformationJobObject(Some(job), JobObjectBasicAccountingInformation, (&raw mut info).cast(), size, None) }.ok()?;
    Some(info.ActiveProcesses)
}

#[cfg(windows)]
impl Drop for Tree {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            // SAFETY: the job handle is ours and closed once.
            let _ = unsafe { windows::Win32::Foundation::CloseHandle(job) };
        }
    }
}
