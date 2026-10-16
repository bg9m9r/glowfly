//! Keeping repository reloads from piling up: at most one load and one status read run for the
//! active tab at a time, and requests made meanwhile are merged into one more run afterwards.

use crate::state::{AppState, MAX_COMMITS};
use trellis_core::repo::Snapshot;

/// How much a refresh reads.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Depth {
    /// The status, plus everything else only if refs, HEAD, config or worktrees changed (or the
    /// load options did) since the snapshot was loaded.
    Checked,
    /// Everything (F5).
    Full,
}

#[derive(Default, Debug)]
pub struct RefreshState {
    /// A refresh asked for while a load was running; it runs when that load is done.
    pub queued: Option<Depth>,
    /// A status read is running for the current load generation.
    pub status_busy: bool,
    /// The status was asked for again while it was being read.
    pub status_queued: bool,
    /// Commits past the limit that Load more added for this tab (until it is switched away).
    pub extra_commits: usize,
    /// The open file's `repo::open_file_stamp` when its diff was last read again after a status
    /// read.
    pub open_stamp: Option<u64>,
}

/// Whether `snap` stops short of the whole history and may load more of it (below the cap). A
/// truncated walk loads exactly `max_commits` commits.
pub fn can_load_more(snap: &Snapshot) -> bool {
    snap.truncated && snap.loaded_with.max_commits < MAX_COMMITS
}

impl AppState {
    /// How many commits the active tab's history walk loads: the limit plus what Load more
    /// added, up to the cap.
    pub fn commits_wanted(&self) -> usize {
        let limit = self.settings.commit_limit();
        (limit + self.refresh.extra_commits).min(MAX_COMMITS.max(limit))
    }

    /// Whether more commits were asked for than the snapshot was loaded with: a load for more is
    /// running or queued.
    pub fn loading_more(&self) -> bool {
        self.snap.as_ref().is_some_and(|s| s.loaded_with.max_commits < self.commits_wanted())
    }

    /// Whether the graph shows its "Loading more commits…" row.
    pub fn shows_loading_more_row(&self) -> bool {
        self.loading && self.loading_more()
    }
}
