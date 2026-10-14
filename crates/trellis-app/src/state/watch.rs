//! The file watcher as `update` sees it: which repository the runner watches, and the grace
//! period after Trellis' own changes.

use std::collections::HashSet;
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
}
