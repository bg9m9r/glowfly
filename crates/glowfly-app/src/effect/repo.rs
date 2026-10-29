//! Data the repository effects carry: index and working-tree edits, what a refresh compares
//! against, the watcher's target, and what a git job means for the undo history.

use git2::RepositoryState;
use glowfly_core::repo::{ConflictIds, FileEntry, Hunk, HunkOp, IndexMemo, PathsStatus, Side};
use glowfly_core::undo::Action;
use std::path::PathBuf;
use std::sync::Arc;

/// Quick index/working-tree edits (through libgit2, or git where filters must run).
#[derive(Debug, PartialEq)]
pub enum LocalOp {
    StageFile(String),
    StageAll,
    UnstagePaths(Vec<String>),
    /// Checks out the index version of a tracked file.
    DiscardFile(String),
    Hunk {
        op: HunkOp,
        path: String,
        index: usize,
        /// The hunk as displayed; the operation is refused if the file's hunk differs now.
        expected: Hunk,
        /// The hunk was shown with whitespace changes hidden; core applies only the shown changes.
        ignore_whitespace: bool,
    },
    /// Lines `lines` (sorted) of hunk `hunk`, whose header must still be `header`.
    Lines {
        op: HunkOp,
        path: String,
        hunk: usize,
        /// The hunk as displayed; the operation is refused if the file's hunk differs now.
        expected: Hunk,
        lines: Vec<usize>,
        /// The hunk was shown with whitespace changes hidden; core applies only the shown changes.
        ignore_whitespace: bool,
    },
    /// Writes a resolved conflict's output and stages it, if the conflict is still `expected`.
    ResolveConflict {
        path: String,
        contents: String,
        expected: ConflictIds,
    },
    /// Writes the working file only (conflict markers left for chunks not picked yet), if the
    /// conflict is still `expected`; the file stays conflicted.
    SavePartialResolution {
        path: String,
        contents: String,
        expected: ConflictIds,
    },
    /// Resolves a conflict with one side's whole version (or its deletion).
    ResolveWithSide {
        path: String,
        side: Side,
        expected: ConflictIds,
    },
    /// Stages conflicted files as they are, marking them resolved.
    MarkResolved(Vec<String>),
    /// Brings back resolved files' conflicts from git's resolve-undo record.
    MarkConflicted(Vec<String>),
}

/// Why the conflicted files' working copies are read (`ConflictEffect::ReadFiles`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConflictRead {
    /// The WIP panel's counts and Resolved Files; results of older generations are dropped.
    Summary { generation: u64 },
    /// The markers check before Mark resolved stages the files.
    BeforeMarkResolved,
}

/// What a status read found (`LoadMsg::StatusLoaded`).
#[derive(Debug)]
pub enum StatusRead {
    /// Everything: (unstaged, staged, state).
    Whole(Vec<FileEntry>, Vec<FileEntry>, RepositoryState),
    /// Only the unstaged rows under some work-tree paths, to merge into the lists on screen.
    Paths(PathsStatus),
    /// After an index change: both lists' rows under `paths.covered` (`repo::status_after_index_change`).
    Index { paths: PathsStatus, staged: Vec<FileEntry> },
}

/// What an index read compares and reads (`Effect::LoadIndexStatus`).
#[derive(Debug)]
pub struct IndexRequest {
    /// The memo of the index the lists on screen were read from.
    pub old: Arc<IndexMemo>,
    /// The staged additions, deletions and renames on screen, with the renames' old paths.
    pub partners: Vec<String>,
    /// The work-tree paths the watcher named with the change.
    pub paths: Vec<String>,
}

/// What a refresh compares against before loading everything again.
#[derive(Clone, Debug, PartialEq)]
pub struct KnownRepo {
    pub refs_key: u64,
    /// The snapshot has submodules: read them again even without a .gitmodules.
    pub submodules: bool,
    /// The file whose working-tree or index diff is open, for `LoadMsg::RepoUnchanged::open_stamp`.
    pub open: Option<String>,
    /// Let git read the status (fsmonitor, untracked cache); false while the repository isn't
    /// trusted.
    pub git_status: bool,
}

/// A repository for the file watcher.
#[derive(Clone, Debug, PartialEq)]
pub struct WatchTarget {
    pub workdir: PathBuf,
    pub git_dir: PathBuf,
}

/// What a git job means for the undo history.
#[derive(Debug)]
pub enum UndoResult {
    /// An undoable job ran; `None` when it changed nothing that can be undone.
    Recorded(Option<Action>),
    /// An undo or redo ran. `before` is the history entry and `after` the entry as it stands now
    /// (they differ for a redo that makes a new stash or leaves a new working tree). `stash`:
    /// what became of the stash "Stash changes and undo" made, when it isn't simply back.
    Done { redo: bool, before: Action, after: Box<Action>, stash: Option<UndoStash> },
    /// A merge, pull, cherry-pick, revert or rebase stopped in progress; it started at `0`.
    Stopped(glowfly_core::undo::OpStart),
    /// A plain abort (`undo::is_abort`) ran; the repository is now at `0` (`None`: something is
    /// still in progress, or it can't be read).
    Aborted(Option<glowfly_core::undo::OpStart>),
    /// The reversing git commands failed (or "Stash changes and undo" couldn't stash); the entry
    /// stays.
    Failed { redo: bool, stash: Option<UndoStash> },
    /// The undo or redo was refused before running anything; a stale history is cleared.
    /// `offer_stash`: the refusal was for local changes that "Stash changes and undo" can move
    /// out of the way.
    Refused { redo: bool, stale: bool, offer_stash: bool, stash: Option<UndoStash> },
}

/// The stash "Stash changes and undo" made (`message`, with its run's nonce), and what became of
/// it; `reason` is the refusal, for an undo that was refused after stashing.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoStash {
    pub message: String,
    pub how: StashHow,
    pub reason: Option<String>,
}

/// What became of the changes "Stash changes and undo" stashed.
#[derive(Clone, Debug, PartialEq)]
pub enum StashHow {
    /// Nothing was undone, and they are back as they were (`pop --index`).
    Back,
    /// They stay in the list: by design (`Action::restores_stash` is false), or because the undo
    /// failed partway.
    Left,
    /// git put them back with conflicts in this many files, and kept the entry.
    Conflicts(usize),
    /// git put back the tracked changes but not these untracked files, and kept the entry.
    Partial(Vec<String>),
    /// git couldn't put them back (also not with their staged state); the entry stays.
    Failed,
    /// The stash was no longer in the list when they were to go back.
    Gone,
    /// Nothing was stashed or undone: git can't stash these intent-to-add files (`git add -N`).
    IntentToAdd(Vec<String>),
}

/// Choices for one undo or redo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UndoOpts {
    /// Stash local changes before planning the undo (`runner/undo_stash.rs`).
    pub stash_first: bool,
    /// Undoing a checkout that created a branch also deletes the branch.
    pub delete_created: bool,
}

/// Whether a repository load opens the repository or refreshes the one on screen.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LoadKind {
    Open,
    Refresh,
    /// A load for more history: goes on from the snapshot's walk (`repo::load_more`, reported as
    /// `LoadMsg::HistoryExtended`) while its refs are as loaded; otherwise a full refresh,
    /// reported as `RepoLoaded` with `Refresh`.
    More,
}
