//! The file watcher as `update` sees it: which repository the runner watches, and the grace
//! period after Trellis' own changes.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Grace after Trellis' own job or local op: watcher events up to then were caused by it.
pub const WATCH_SETTLE: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct WatchState {
    /// Workdir the runner was last asked to watch (None: not watching).
    pub target: Option<PathBuf>,
    /// `settings.watch_files` as last applied (to notice the Preferences toggle).
    pub enabled: bool,
    /// Bursts whose last event is at or before this were caused by Trellis itself.
    pub quiet_until: Option<Instant>,
    /// Repositories whose watcher failure was already reported this session.
    pub failed: HashSet<PathBuf>,
    /// Repositories on a network or virtual file system, with its kind (said once each).
    pub remote: HashMap<PathBuf, &'static str>,
    /// When the status was last re-read on the timer (see `poll_every`).
    pub last_poll: Option<Instant>,
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

impl WatchState {
    /// `poll_every` for the watched repository on this system.
    pub fn poll_interval(&self) -> Option<Duration> {
        let target = self.target.as_ref()?;
        poll_every(true, cfg!(windows), self.remote.contains_key(target))
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
}
