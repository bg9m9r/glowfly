//! Keeping repository reloads from piling up: at most one load and one status read run for the
//! active tab at a time, and requests made meanwhile are merged into one more run afterwards.

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
}
