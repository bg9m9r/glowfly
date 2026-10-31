//! Keeping repository reloads from piling up: at most one load and one status read run for the
//! active tab at a time, and requests made meanwhile are merged into one more run afterwards.
//! It also holds a failed load for more history with its automatic retries, and a filtering
//! search loading further back (`load_more.rs`).

pub mod recount;

use crate::state::load_more::DeeperSearch;
use crate::state::{AppState, MAX_COMMITS};
use glowfly_core::repo::{LoadOptions, Snapshot};
use glowfly_core::watch::MAX_WATCH_PATHS;
use std::collections::BTreeSet;
use std::time::Instant;

/// How much a refresh reads.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Depth {
    /// Only more of the history; read like `Checked` when it can't go on from the walk.
    More,
    /// The status, plus everything else only if refs, HEAD, config or worktrees changed (or the
    /// load options did) since the snapshot was loaded.
    Checked,
    /// Everything (F5).
    Full,
}

/// What a status read waiting for the running one reads.
#[derive(Debug, Clone, PartialEq)]
pub enum Wanted {
    /// The whole status.
    Whole,
    /// Only the unstaged rows under these work-tree paths (the file watcher's).
    Paths(BTreeSet<String>),
    /// The rows the index change can have changed, with these work-tree paths (an index read).
    Index(BTreeSet<String>),
}

impl Wanted {
    /// One read covering both: a whole read absorbs anything, path sets unite (an index read
    /// with them stays one), and a union past `MAX_WATCH_PATHS` becomes a whole read.
    pub fn merge(self, other: Wanted) -> Wanted {
        let (index, mut a, b) = match (self, other) {
            (Wanted::Paths(a), Wanted::Paths(b)) => (false, a, b),
            (Wanted::Index(a) | Wanted::Paths(a), Wanted::Index(b) | Wanted::Paths(b)) => (true, a, b),
            _ => return Wanted::Whole,
        };
        a.extend(b);
        match (a.len() > MAX_WATCH_PATHS, index) {
            (true, _) => Wanted::Whole,
            (false, true) => Wanted::Index(a),
            (false, false) => Wanted::Paths(a),
        }
    }

    /// `queued` merged with `more`.
    pub fn queue(queued: Option<Wanted>, more: Wanted) -> Wanted {
        match queued {
            Some(q) => q.merge(more),
            None => more,
        }
    }
}

#[derive(Default, Debug)]
pub struct RefreshState {
    /// A refresh asked for while a load was running; it runs when that load is done.
    pub queued: Option<Depth>,
    /// A status read is running for the current load generation.
    pub status_busy: bool,
    /// The status was asked for again while it was being read: what the next read covers.
    pub status_queued: Option<Wanted>,
    /// Commits past the limit that Load more added for this tab (until it is switched away).
    pub extra_commits: usize,
    /// The open file's `repo::open_file_stamp` when its diff was last read again after a status
    /// read.
    pub open_stamp: Option<u64>,
    /// The `refs_key` a background count of other worktrees' changes runs for; one at a time.
    /// Kept across loads (`refresh::start`), reset by a tab switch.
    pub counting: Option<u64>,
    /// When other worktrees' changes are counted again after a reload that kept the snapshot.
    /// Kept across loads like `counting`; a tab switch resets it, so returning counts at once.
    pub recount: recount::Recount,
    /// The error of the last load that asked for more history, until a load starts.
    pub more_failed: Option<String>,
    /// When the next automatic retry of that load is due.
    pub more_retry_at: Option<Instant>,
    /// Automatic retries since a load last succeeded or more was asked for; carried across
    /// loads.
    pub more_retries: u8,
    /// The filtering search loading older history by itself; carried across loads.
    pub deeper: Option<DeeperSearch>,
    /// The graph layout asked of a worker for a large history (update/large_graph.rs), if any.
    pub relayout: Option<u64>,
    /// Counts those requests.
    pub relayouts: u64,
    /// The load for more running now was also asked to check the status (a refresh merged into
    /// it): read it when the page lands.
    pub status_after: bool,
}

/// From this many loaded commits, whole-history work runs off the UI thread: matching a typed
/// search, laying the graph out again, the default branch's ancestry.
pub const LARGE_HISTORY: usize = 20_000;

/// Whether `snap` holds a large history (`LARGE_HISTORY`).
pub fn large(snap: &Snapshot) -> bool {
    snap.commits.len() >= LARGE_HISTORY
}

/// Whether `snap` stops short of the whole history and may load more of it (below the cap). A
/// truncated walk loads exactly `max_commits` commits.
pub fn can_load_more(snap: &Snapshot) -> bool {
    snap.truncated && snap.loaded_with.max_commits < MAX_COMMITS
}

/// Whether loading with `opts` only asks `snap` for more of its history: its walk was cut short
/// and every other option is as it was loaded with.
pub fn only_more(snap: &Snapshot, opts: &LoadOptions) -> bool {
    snap.frontier.is_some()
        && opts.max_commits > snap.loaded_with.max_commits
        && LoadOptions { max_commits: snap.loaded_with.max_commits, ..opts.clone() } == snap.loaded_with
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

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(p: &[&str]) -> Wanted {
        Wanted::Paths(p.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn queued_reads_merge() {
        assert_eq!(paths(&["a", "b"]).merge(paths(&["b", "c"])), paths(&["a", "b", "c"]));
        assert_eq!(paths(&["a"]).merge(Wanted::Whole), Wanted::Whole);
        assert_eq!(Wanted::Whole.merge(paths(&["a"])), Wanted::Whole);
        assert_eq!(Wanted::Whole.merge(Wanted::Whole), Wanted::Whole);
        assert_eq!(Wanted::queue(None, paths(&["a"])), paths(&["a"]));
        assert_eq!(Wanted::queue(Some(paths(&["a"])), paths(&["b"])), paths(&["a", "b"]));
        let half = |from: usize| Wanted::Paths((from..from + MAX_WATCH_PATHS / 2 + 1).map(|i| format!("f{i}")).collect());
        assert_eq!(half(0).merge(half(MAX_WATCH_PATHS)), Wanted::Whole, "over the cap");
        let exact = Wanted::Paths((0..MAX_WATCH_PATHS).map(|i| format!("f{i}")).collect());
        assert!(matches!(exact.merge(paths(&["f0"])), Wanted::Paths(p) if p.len() == MAX_WATCH_PATHS));
    }

    #[test]
    fn queued_index_reads_merge() {
        let index = |p: &[&str]| Wanted::Index(p.iter().map(|s| s.to_string()).collect());
        assert_eq!(index(&["a"]).merge(index(&["b"])), index(&["a", "b"]));
        assert_eq!(index(&["a"]).merge(paths(&["b"])), index(&["a", "b"]), "Index + Paths");
        assert_eq!(paths(&["a"]).merge(index(&[])), index(&["a"]), "Paths + Index");
        assert_eq!(index(&["a"]).merge(Wanted::Whole), Wanted::Whole);
        assert_eq!(Wanted::Whole.merge(index(&["a"])), Wanted::Whole);
        let half = |from: usize| Wanted::Index((from..from + MAX_WATCH_PATHS / 2 + 1).map(|i| format!("f{i}")).collect());
        assert_eq!(half(0).merge(half(MAX_WATCH_PATHS)), Wanted::Whole, "over the cap");
    }
}
