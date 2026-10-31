//! Keeping a built-in terminal's repository out of the repository pool while a command typed
//! there runs. On Windows a pooled handle keeps the pack files it has open, so a `git gc` or
//! `git repack` run in the terminal while Glowfly reads would leave the old packs behind.
//!
//! What the user types is never read: Enter (a carriage return or line feed in what is written to
//! the shell) means a command may start. The pool then drops the repository's idle handles and
//! keeps it out (`repo_pool::hold`) for as long as the shell's activity probe says it is running
//! a program, checked a few times a second on a thread of the session's own; at most one such
//! thread runs per session. A program that starts a moment after Enter is covered by a short
//! grace period, and while the shell can't be read (Unknown) only the grace period holds.

use glowfly_core::terminal::{ShellActivity, ShellProbe};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long after Enter the repository is held even while the shell still looks idle.
const GRACE: Duration = Duration::from_millis(750);
/// How often the shell's activity is read while the repository is held.
const POLL: Duration = Duration::from_millis(250);

/// Whether `bytes` written to a shell end a line: the shell may now run a command.
pub(super) fn runs_command(bytes: &[u8]) -> bool {
    bytes.iter().any(|&b| b == b'\r' || b == b'\n')
}

/// One session's hold: the folder its shell started in, and whether a thread is holding it.
#[derive(Default)]
pub(super) struct CommandHold {
    cwd: PathBuf,
    holding: AtomicBool,
}

impl CommandHold {
    pub(super) fn new(cwd: PathBuf) -> Self {
        Self { cwd, holding: AtomicBool::new(false) }
    }

    /// Enter was written to the shell: holds the repository until the shell is idle again,
    /// unless a hold is already running. `probe` is None while the shell is still starting.
    pub(super) fn command_started(self: &Arc<Self>, probe: Option<ShellProbe>) {
        if self.cwd.as_os_str().is_empty() || self.holding.swap(true, Ordering::AcqRel) {
            return;
        }
        let me = self.clone();
        let spawned = std::thread::Builder::new().name("glowfly-terminal-hold".into()).spawn(move || {
            let _held = glowfly_core::repo_pool::hold(&me.cwd);
            let activity = || probe.as_ref().map_or(ShellActivity::Unknown, ShellProbe::activity);
            wait_while_busy(activity, Instant::now(), std::thread::sleep);
            me.holding.store(false, Ordering::Release);
        });
        if spawned.is_err() {
            self.holding.store(false, Ordering::Release);
        }
    }
}

/// Returns once the grace period after `start` is over and `activity` no longer says Running,
/// sleeping `POLL` between reads with `sleep`.
fn wait_while_busy(mut activity: impl FnMut() -> ShellActivity, start: Instant, mut sleep: impl FnMut(Duration)) {
    let mut slept = Duration::ZERO;
    loop {
        let running = matches!(activity(), ShellActivity::Running { .. });
        if !running && start.elapsed().max(slept) >= GRACE {
            return;
        }
        sleep(POLL);
        slept += POLL;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_starts_a_command() {
        assert!(runs_command(b"git gc\r"));
        assert!(runs_command(b"\n"));
        assert!(runs_command(b"one\r\ntwo"));
        assert!(!runs_command(b"git gc"));
        assert!(!runs_command(b"\x1b[A"));
    }

    /// How many times `wait_while_busy` read the activity, given the answers in `script`
    /// (the last repeats), with a clock that only moves when it sleeps.
    fn reads(script: &[ShellActivity]) -> usize {
        let mut n = 0;
        let start = Instant::now();
        wait_while_busy(
            || {
                n += 1;
                script[(n - 1).min(script.len() - 1)].clone()
            },
            start,
            |_| {},
        );
        n
    }

    #[test]
    fn the_hold_lasts_while_a_program_runs() {
        let running = ShellActivity::Running { program: Some("git".into()) };
        let grace_reads = (GRACE.as_millis() / POLL.as_millis()) as usize + 1;
        // Idle from the start: only the grace period.
        assert_eq!(reads(&[ShellActivity::Idle]), grace_reads);
        assert_eq!(reads(&[ShellActivity::Unknown]), grace_reads, "unreadable: the grace period only");
        // A program that runs past the grace period is waited for.
        let mut script = vec![running.clone(); grace_reads + 5];
        script.push(ShellActivity::Idle);
        assert_eq!(reads(&script), grace_reads + 6);
    }

    #[test]
    fn one_hold_per_session_and_none_without_a_folder() {
        let hold = Arc::new(CommandHold::default());
        hold.command_started(None);
        assert!(!hold.holding.load(Ordering::Acquire), "no folder: nothing to hold");
        let dir = glowfly_core::paths::temp_dir();
        let hold = Arc::new(CommandHold::new(dir));
        hold.command_started(None);
        assert!(hold.holding.load(Ordering::Acquire));
        // A second Enter while it holds starts no second thread; the first ends after the grace.
        hold.command_started(None);
        let deadline = Instant::now() + Duration::from_secs(10);
        while hold.holding.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!hold.holding.load(Ordering::Acquire), "released after the grace period");
    }
}
