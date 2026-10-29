//! Messages for opening and reloading the repository, the selection, diffs and the file watcher
//! (update/repo.rs, update/refresh.rs, update/watch.rs).

use crate::effect::LoadKind;
use crate::effect::StatusRead;
use git2::Oid;
use glowfly_core::repo::{CommitDetail, DiffTarget, FileDiff, FileEntry, FileLineCounts, IndexMemo, SlowStatus, Snapshot, WipLineCounts};
use glowfly_core::signature_check::SignatureCheck;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Opening the repository, selecting a commit or the working tree, opening a diff, and their
/// results (update/repo.rs).
pub enum RepoMsg {
    PickRepo,
    Open(PathBuf),
    Refresh,
    WindowFocused,
    SelectWip,
    /// The in-progress strip's click: select the WIP row and show the details panel.
    ShowInProgress,
    SelectCommit(Oid),
    JumpTo(Oid),
    OpenDiff(DiffTarget),
    CloseDiff,
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
        /// The two sides when either is a Git LFS pointer (`repo::ViewDiff::lfs`); None on an error.
        lfs: Option<glowfly_core::lfs::pointer::LfsChange>,
        /// The hunk's exact bytes, hashed (`repo::ViewDiff::raw`); None on an error.
        raw: Option<Oid>,
    },
    StatsLoaded(Vec<(Oid, usize, usize, usize)>),
    /// Stats asked for but not worked out: a newer request came first (the graph scrolled on).
    /// They can be asked for again.
    StatsDropped(Vec<Oid>),
    /// The details panel shows a signed commit whose check isn't known or on its way (sent by
    /// the view, like WantStats).
    WantSignature(Oid),
    /// Forget the cached check of this commit and run it again (badge menu).
    CheckSignatureAgain(Oid),
    SignatureChecked {
        oid: Oid,
        generation: u64,
        result: Result<SignatureCheck, String>,
    },
    /// Commit `oid`'s per-file line counts (update/line_counts.rs).
    CommitLineCountsLoaded {
        oid: Oid,
        result: Result<Arc<FileLineCounts>, String>,
    },
    /// The working tree's per-file line counts for request `generation`.
    WipLineCountsLoaded {
        generation: u64,
        result: Result<Box<WipLineCounts>, String>,
    },
}

impl RepoMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::PickRepo
            | Self::Open(..)
            | Self::Refresh
            | Self::WindowFocused
            | Self::SelectWip
            | Self::ShowInProgress
            | Self::SelectCommit(..)
            | Self::JumpTo(..)
            | Self::OpenDiff(..)
            | Self::CloseDiff
            | Self::WantStats(..)
            | Self::InitRepo
            | Self::RepoLoaded { .. }
            | Self::DetailLoaded { .. }
            | Self::DiffLoaded { .. }
            | Self::StatsLoaded(..)
            | Self::StatsDropped(..)
            // Dropped: parking forgot the pending check, so the view asks again on return.
            | Self::WantSignature(..)
            | Self::CheckSignatureAgain(..)
            | Self::SignatureChecked { .. }
            // Dropped: parking forgot the pending request, and the return asks again.
            | Self::CommitLineCountsLoaded { .. }
            | Self::WipLineCountsLoaded { .. } => None,
        }
    }
}

/// A refresh's results, and loading more of the history (update/refresh.rs).
pub enum LoadMsg {
    /// Show more of the history than the commit limit (the graph's "Load more"); while a
    /// failure is held, tries the same load again.
    LoadMoreCommits,
    /// Try again, in the failed load's row and the no-matches pane: loads the held failed load
    /// again at once.
    RetryLoadMore,
    /// Keep searching, in the no-matches pane: gives the filtering search a fresh batch budget.
    SearchFurther,
    /// The graph was scrolled near the last loaded commit (pushed by the view each frame it is
    /// there and nothing is pending); loads more when the preference allows.
    NearHistoryEnd,
    /// A refresh found refs, HEAD, config and worktrees as the snapshot has them: only the
    /// status was read (as for `StatusLoaded`).
    RepoUnchanged {
        generation: u64,
        result: Result<(Vec<FileEntry>, Vec<FileEntry>, git2::RepositoryState), String>,
        submodules: Option<Vec<glowfly_core::repo::SubmoduleInfo>>,
        open_stamp: Option<u64>,
        /// The status read was slow (`repo::SLOW_STATUS`), and what could speed it up.
        slow: Option<SlowStatus>,
        /// The root .gitattributes' LFS patterns, read with a whole status; None for a read of some paths.
        lfs_patterns: Option<Vec<glowfly_core::lfs::LfsPattern>>,
        /// The index the whole or index read was taken from.
        index: Option<Arc<IndexMemo>>,
    },
    /// A load for more went on from the walk: the next commits, or why reading them failed.
    HistoryExtended { generation: u64, result: Result<Box<glowfly_core::repo::HistoryPage>, String> },
    StatusLoaded {
        generation: u64,
        /// The whole status, or only some work-tree paths' unstaged rows.
        result: Result<StatusRead, String>,
        /// The submodules read again; None when the repository has none (or they couldn't be read).
        submodules: Option<Vec<glowfly_core::repo::SubmoduleInfo>>,
        /// `repo::open_file_stamp` of the file asked about; while it stays the same, the open
        /// diff isn't read again.
        open_stamp: Option<u64>,
        /// A whole read was slow (`repo::SLOW_STATUS`), and what could speed it up.
        slow: Option<SlowStatus>,
        /// The root .gitattributes' LFS patterns, read with a whole status; None for a read of some paths.
        lfs_patterns: Option<Vec<glowfly_core::lfs::LfsPattern>>,
        /// The index the whole or index read was taken from.
        index: Option<Arc<IndexMemo>>,
    },
    /// The slow-status note's Turn on, for the active repository: what the note offered (the
    /// untracked cache, git's built-in file system monitor) and the speed-ups set when it showed.
    SpeedUpStatus { speedups: glowfly_core::repo::StatusSpeedups, cache: bool, monitor: bool },
    /// The file system monitor job ended (runner/monitor.rs); unscoped, it ends the active job.
    /// `cache`: None when the untracked cache wasn't asked for, else whether its commands worked.
    StatusSpedUp { output: String, monitor: glowfly_core::repo::MonitorOutcome, cache: Option<bool> },
    /// Repository > Hooks' Turn off for git's file system monitor, in the active repository.
    TurnOffMonitor,
    /// The Turn off job ended (runner/monitor.rs); unscoped, it ends the active job. `ok`: the
    /// setting is unset and no daemon watches.
    MonitorTurnedOff { output: String, ok: bool },
}

impl LoadMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::LoadMoreCommits
            | Self::RetryLoadMore
            | Self::SearchFurther
            | Self::NearHistoryEnd
            | Self::RepoUnchanged { .. }
            // Dropped: the tab loads again when it is shown.
            | Self::HistoryExtended { .. }
            | Self::StatusLoaded { .. }
            | Self::SpeedUpStatus { .. }
            | Self::StatusSpedUp { .. }
            | Self::TurnOffMonitor
            | Self::MonitorTurnedOff { .. } => None,
        }
    }
}

/// The file watcher's reports (update/watch.rs).
pub enum WatchMsg {
    /// Files changed outside Glowfly (debounced). Sent wrapped in `Msg::Scoped` for the watched
    /// repository; `full` when HEAD, refs or a merge/rebase state changed; `paths`: the
    /// work-tree paths that changed, when nothing else did; None re-reads the whole status;
    /// `last_event` is when the burst's last relevant event arrived.
    RepoChanged { full: bool, paths: Option<Vec<String>>, last_event: Instant },
    /// The index changed (debounced, `Msg::Scoped` as `RepoChanged`); `paths`: work-tree paths
    /// named in the same burst.
    IndexChanged { paths: Vec<String>, last_event: Instant },
    /// The repository's config (`config`, `config.worktree`, the common dir's `config` or a file
    /// they include) changed outside Glowfly, right after the `RepoChanged` it came with
    /// (`Msg::Scoped` as `RepoChanged`): the identity commits get may differ now.
    ConfigChanged,
    /// The file watcher could not start or stopped working for `workdir`.
    WatchFailed { workdir: PathBuf, error: String },
    /// `workdir` is on a network or virtual file system of this `kind` ("SMB", "WSL"…), which
    /// may not report changes made elsewhere.
    RemoteVolume { workdir: PathBuf, kind: &'static str },
}

impl WatchMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::RepoChanged { .. }
            | Self::IndexChanged { .. }
            | Self::ConfigChanged
            | Self::WatchFailed { .. }
            | Self::RemoteVolume { .. } => None,
        }
    }
}
