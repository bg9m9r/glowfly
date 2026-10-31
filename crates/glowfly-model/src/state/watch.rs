//! The file watcher as `update` sees it: which repository the runner watches, the grace
//! period after Glowfly's own changes, and whether its probe showed that events arrive (the
//! status bar's live-updates dot, `LiveUpdates`).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Grace after Glowfly's own job or local op: watcher events up to then were caused by it.
pub const WATCH_SETTLE: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct WatchState {
    /// Workdir the runner was last asked to watch (None: not watching).
    pub target: Option<PathBuf>,
    /// `settings.watch_files` as last applied (to notice the Preferences toggle).
    pub enabled: bool,
    /// Bursts whose last event is at or before this were caused by Glowfly itself.
    pub quiet_until: Option<Instant>,
    /// Repositories whose watcher failure was already reported this session.
    pub failed: HashSet<PathBuf>,
    /// Repositories on a network or virtual file system, with its kind (said once each).
    pub remote: HashMap<PathBuf, &'static str>,
    /// When the status was last re-read on the timer (see `poll_every`).
    pub last_poll: Option<Instant>,
    /// What each repository's current watcher probe found (cleared when its watcher restarts
    /// or fails).
    pub probes: HashMap<PathBuf, Probe>,
    /// Repositories whose current watcher failed (cleared when it restarts), unlike `failed`,
    /// which remembers for the session that it was said.
    pub down: HashSet<PathBuf>,
    /// Repositories whose silent watcher was already reported this session.
    pub silent_warned: HashSet<PathBuf>,
}

/// What the watcher's probe (`glowfly_core::watch_probe`) found: its file's event arrived, it
/// didn't within `PROBE_TIMEOUT`, or no probe could be placed (a read-only git dir).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Probe {
    Live,
    Silent,
    Unknown,
}

/// How long the watcher waits for its probe's event before it calls the watcher silent.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// What the status bar's live-updates dot says about the watched repository.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiveUpdates {
    /// The watcher started; its probe hasn't answered yet.
    Checking,
    Live,
    /// Events don't arrive: the status is re-read on the timer instead.
    Silent,
    /// A read-only repository: no probe, so nobody knows.
    Unknown,
    /// The watcher couldn't start or stopped.
    Failed,
}

/// How often a repository on a network or virtual file system is re-read.
pub const REMOTE_POLL: Duration = Duration::from_secs(10);
/// How often a Windows repository is re-read anyway: notify drops events without saying so when
/// its buffer overflows, and stops watching on other errors with only a log line.
pub const WINDOWS_POLL: Duration = Duration::from_secs(30);

/// How often the open repository's status is re-read while the window is focused, as a safety
/// net for the watcher; None when the watcher is trusted, or watching is off (then focus and F5
/// refresh, as the preference says).
pub fn poll_every(watching: bool, windows: bool, remote: bool) -> Option<Duration> {
    match (watching, remote, windows) {
        (false, _, _) => None,
        (true, true, _) => Some(REMOTE_POLL),
        (true, false, true) => Some(WINDOWS_POLL),
        (true, false, false) => None,
    }
}

/// Whether the poll sweeps the tree (`Effect::SweepWatch`) rather than reading the whole status:
/// on Windows, for a local repository whose watcher hasn't failed.
fn sweep_poll(windows: bool, remote: bool, failed: bool) -> bool {
    windows && !remote && !failed
}

impl WatchState {
    /// `poll_every` for the watched repository on this system; a watcher whose probe went
    /// unanswered is polled like one on a network file system.
    pub fn poll_interval(&self) -> Option<Duration> {
        let target = self.target.as_ref()?;
        let silent = self.probes.get(target) == Some(&Probe::Silent);
        poll_every(true, cfg!(windows), self.remote.contains_key(target) || silent)
    }

    /// The live-updates dot for the watched repository; None while nothing is watched.
    pub fn live_updates(&self) -> Option<LiveUpdates> {
        let target = self.target.as_ref()?;
        Some(match self.probes.get(target) {
            Some(Probe::Live) => LiveUpdates::Live,
            Some(Probe::Silent) => LiveUpdates::Silent,
            Some(Probe::Unknown) => LiveUpdates::Unknown,
            None if self.down.contains(target) => LiveUpdates::Failed,
            None => LiveUpdates::Checking,
        })
    }

    /// `sweep_poll` for the watched repository on this system.
    pub fn sweeps(&self) -> bool {
        self.target.as_ref().is_some_and(|t| sweep_poll(cfg!(windows), self.remote.contains_key(t), self.failed.contains(t)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polling_backs_up_the_watcher_where_it_can_be_silent() {
        assert_eq!(poll_every(true, false, false), None, "inotify and FSEvents report overflows");
        assert_eq!(poll_every(true, true, false), Some(WINDOWS_POLL));
        assert_eq!(poll_every(true, false, true), Some(REMOTE_POLL));
        assert_eq!(poll_every(true, true, true), Some(REMOTE_POLL));
        assert_eq!(poll_every(false, true, true), None, "watching is off: focus and F5 only");
    }

    #[test]
    fn a_silent_probe_polls_and_the_dot_follows_the_probe() {
        let repo = PathBuf::from("/r");
        let mut w = WatchState { target: Some(repo.clone()), ..WatchState::default() };
        assert_eq!(w.live_updates(), Some(LiveUpdates::Checking));
        w.probes.insert(repo.clone(), Probe::Silent);
        assert_eq!(w.live_updates(), Some(LiveUpdates::Silent));
        assert_eq!(w.poll_interval(), Some(REMOTE_POLL));
        w.probes.insert(repo.clone(), Probe::Live);
        assert_eq!(w.poll_interval(), poll_every(true, cfg!(windows), false));
        w.probes.clear();
        w.down.insert(repo);
        assert_eq!(w.live_updates(), Some(LiveUpdates::Failed));
        w.target = None;
        assert_eq!(w.live_updates(), None, "nothing watched: no dot");
    }

    #[test]
    fn only_a_local_windows_watcher_is_swept() {
        assert!(sweep_poll(true, false, false));
        assert!(!sweep_poll(true, true, false), "a network volume reads everything");
        assert!(!sweep_poll(true, false, true), "a failed watcher can't sweep");
        assert!(!sweep_poll(false, false, false), "no poll elsewhere");
    }
}
