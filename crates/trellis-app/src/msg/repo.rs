//! Messages for opening and reloading the repository, the selection, diffs and the file watcher
//! (update/repo.rs, update/refresh.rs, update/watch.rs).

use crate::effect::LoadKind;
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{CommitDetail, DiffTarget, FileDiff, FileEntry, Snapshot};

/// Opening the repository, selecting a commit or the working tree, opening a diff, and their
/// results (update/repo.rs).
pub enum RepoMsg {
    PickRepo,
    Open(PathBuf),
    Refresh,
    WindowFocused,
    SelectWip,
    SelectCommit(Oid),
    JumpTo(Oid),
    OpenDiff(DiffTarget),
    CloseDiff,
    SearchNext,
    /// The graph shows these commits and needs their line counts.
    WantStats(Vec<Oid>),
    InitRepo,
    RepoLoaded {
        generation: u64,
        kind: LoadKind,
        result: Result<Box<Snapshot>, String>,
    },
    DetailLoaded {
        oid: Oid,
        result: Result<CommitDetail, String>,
    },
    DiffLoaded {
        target: DiffTarget,
        reload: bool,
        generation: u64,
        result: Result<FileDiff, String>,
    },
    StatsLoaded(Vec<(Oid, usize, usize, usize)>),
    /// Stats asked for but not worked out: a newer request came first (the graph scrolled on).
    /// They can be asked for again.
    StatsDropped(Vec<Oid>),
}

/// A refresh's results, and loading more of the history (update/refresh.rs).
pub enum LoadMsg {
    /// Show more of the history than the commit limit (the graph's "Load more").
    LoadMoreCommits,
    /// A refresh found refs, HEAD, config and worktrees as the snapshot has them: only the
    /// status was read (as for `StatusLoaded`).
    RepoUnchanged {
        generation: u64,
        result: Result<(Vec<FileEntry>, Vec<FileEntry>, git2::RepositoryState), String>,
        submodules: Option<Vec<trellis_core::repo::SubmoduleInfo>>,
        open_stamp: Option<u64>,
    },
    StatusLoaded {
        generation: u64,
        result: Result<(Vec<FileEntry>, Vec<FileEntry>, git2::RepositoryState), String>,
        /// The submodules read again; None when the repository has none (or they couldn't be read).
        submodules: Option<Vec<trellis_core::repo::SubmoduleInfo>>,
        /// `repo::open_file_stamp` of the file asked about; while it stays the same, the open
        /// diff isn't read again.
        open_stamp: Option<u64>,
    },
}

/// The file watcher's reports (update/watch.rs).
pub enum WatchMsg {
    /// Files changed outside Trellis (debounced). Sent wrapped in `Msg::Scoped` for the watched
    /// repository; `full` when HEAD, refs or a merge/rebase state changed; `last_event` is when
    /// the burst's last relevant event arrived.
    RepoChanged { full: bool, last_event: Instant },
    /// The file watcher could not start or stopped working for `workdir`.
    WatchFailed { workdir: PathBuf, error: String },
}
