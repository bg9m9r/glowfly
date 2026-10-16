//! Messages for staging, committing and running git jobs (update/ops.rs, update/commit.rs,
//! update/undo.rs, update/stash.rs).

use super::Inactive;
use crate::effect::{UndoOpts, UndoResult};
use git2::Oid;
use trellis_core::repo::HunkOp;

/// Staging, unstaging and discarding files, hunks and lines, reverting a commit's hunk, and their
/// results (update/ops.rs).
pub enum StageMsg {
    StageFile(String),
    UnstageFile(String),
    StageAll,
    UnstageAll,
    /// Revert tracked `path` to its index version (the view has confirmed).
    DiscardFile(String),
    /// Stage, unstage or discard hunk `index` of the diff shown at `generation`
    /// (`AppState::hunk_ops_gen`); refused if the diff has changed since.
    Hunk {
        op: HunkOp,
        path: String,
        index: usize,
        generation: u64,
    },
    /// Stage, unstage or discard the selected diff lines. Reads the selection when handled,
    /// so a confirmation whose diff changed meanwhile does nothing.
    ApplyLines(HunkOp),
    /// Revert hunk `index` of the commit or comparison diff shown at `generation`
    /// (AppState::hunk_ops_gen) into the working tree. Refused if the diff changed since.
    RevertHunk {
        index: usize,
        generation: u64,
    },
    /// A LocalOp finished; a discard that can be undone carries its undo action.
    LocalOpDone(Result<Option<trellis_core::undo::Action>, String>),
    /// A RevertHunk finished; `path` is the file it was for.
    HunkReverted {
        path: String,
        result: Result<(), String>,
    },
}

impl StageMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::StageFile(..)
            | Self::UnstageFile(..)
            | Self::StageAll
            | Self::UnstageAll
            | Self::DiscardFile(..)
            | Self::Hunk { .. }
            | Self::ApplyLines(..)
            | Self::RevertHunk { .. }
            | Self::LocalOpDone(Ok(None))
            | Self::HunkReverted { result: Ok(()), .. } => None,
            Self::LocalOpDone(Err(e)) => Some(Inactive::Log(e)),
            // A discard in a tab switched away from: its undo belongs to that tab.
            Self::LocalOpDone(Ok(Some(action))) => Some(Inactive::Undo(action)),
            Self::HunkReverted { path, result: Err(e) } => Some(Inactive::Log(format!("Could not revert the hunk in {path}: {e}"))),
        }
    }
}

/// The commit box (update/commit.rs).
pub enum CommitMsg {
    Commit,
    /// Ctrl+Shift+Enter in the commit message: stage everything (git add --all) and commit in one job.
    StageAllAndCommit,
    /// The commit box's "Push after committing" checkbox (saved in settings).
    SetPushAfterCommit(bool),
    PrefillAmend,
    AppendToDescription(String),
}

impl CommitMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Commit | Self::StageAllAndCommit | Self::SetPushAfterCommit(..) | Self::PrefillAmend | Self::AppendToDescription(..) => {
                None
            }
        }
    }
}

/// Git commands run as the active job, undo and redo, and the job's end (update/ops.rs,
/// update/undo.rs, update/stash.rs).
pub enum JobMsg {
    Git {
        label: String,
        cmds: Vec<Vec<String>>,
    },
    /// Applies, pops or drops the stash shown at `stash@{index}`, only if that entry still is
    /// commit `oid` (another tab, worktree or terminal may have pushed or dropped one since).
    Stash {
        action: crate::state::palette::StashAction,
        index: usize,
        oid: Oid,
    },
    Push {
        force: bool,
    },
    /// Fetch more of a shallow clone's history from its remote (update/shallow.rs).
    Deepen(trellis_core::shallow_ops::Deepen),
    /// Undo the active tab's most recent undoable operation.
    Undo,
    Redo,
    /// Runs the undo (or redo) of `action` with `opts`, only while it is still the top of that
    /// stack: the undo-created-branch dialog and the "Stash changes and undo" toast send it.
    UndoWith {
        redo: bool,
        action: Box<trellis_core::undo::Action>,
        opts: UndoOpts,
    },
    JobDone {
        ok: bool,
        output: String,
        undo: Option<UndoResult>,
    },
}

impl JobMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Git { .. }
            | Self::Stash { .. }
            | Self::Push { .. }
            | Self::Deepen(..)
            | Self::Undo
            | Self::Redo
            | Self::UndoWith { .. }
            | Self::JobDone { .. } => None,
        }
    }
}
