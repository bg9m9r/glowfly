//! Data the repository effects carry: index and working-tree edits, what a refresh compares
//! against, the watcher's target, and what a git job means for the undo history.

use git2::RepositoryState;
use std::path::PathBuf;
use trellis_core::repo::{ConflictIds, FileEntry, Hunk, HunkOp, PathsStatus, Side};
use trellis_core::undo::Action;

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
    },
    /// Lines `lines` (sorted) of hunk `hunk`, whose header must still be `header`.
    Lines {
        op: HunkOp,
        path: String,
        hunk: usize,
        /// The hunk as displayed; the operation is refused if the file's hunk differs now.
        expected: Hunk,
        lines: Vec<usize>,
    },
    /// Writes a resolved conflict's output and stages it, if the conflict is still `expected`.
    ResolveConflict {
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

/// Why the conflicted files' working copies are read (`Effect::ReadConflictFiles`).
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
    /// (they differ only for a redone discard, which makes a new stash).
    Done { redo: bool, before: Action, after: Action },
    /// The reversing git commands failed; the entry stays.
    Failed { redo: bool },
    /// The undo or redo was refused before running anything; a stale history is cleared.
    Refused { redo: bool, stale: bool },
}
