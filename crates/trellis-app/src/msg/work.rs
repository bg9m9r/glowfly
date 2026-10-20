//! Messages for staging, committing and running git jobs (update/ops.rs, update/commit.rs,
//! update/undo.rs, update/stash.rs).

use super::Inactive;
use crate::effect::{UndoOpts, UndoResult};
use git2::Oid;
use trellis_core::repo::{DiffTarget, HunkOp, RevertPart, Reverted};
use trellis_core::undo::Discarded;

/// Staging, unstaging and discarding files, hunks and lines, reverting a hunk, some lines or a
/// whole file of a commit or comparison diff, and their results (update/ops.rs; reverts in
/// update/revert.rs).
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
    /// Revert the selected lines of the open commit or comparison diff into the working tree.
    /// Reads the selection when handled, like ApplyLines.
    RevertLines,
    /// Revert the whole change of a commit (`Commit`) or comparison (`Between`) to one file,
    /// from a file row's menu.
    RevertFile(DiffTarget),
    /// A LocalOp finished; a discard or a saved conflict resolution that can be undone carries
    /// its undo action.
    LocalOpDone(Result<Option<trellis_core::undo::Action>, String>),
    /// A revert finished; `path` is the working-tree file it changed. A revert that changed it
    /// carries the undo action that puts the earlier working copy back.
    Reverted {
        path: String,
        what: RevertWhat,
        result: Result<Reverted, String>,
        undo: Option<trellis_core::undo::Action>,
    },
}

/// What a revert undid, for its toast and log texts.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RevertWhat {
    Hunk,
    /// This many selected lines.
    Lines(usize),
    File,
}

impl RevertWhat {
    pub fn of(part: &RevertPart) -> Self {
        match part {
            RevertPart::Hunk { .. } => Self::Hunk,
            RevertPart::Lines { lines, .. } => Self::Lines(lines.len()),
            RevertPart::File => Self::File,
        }
    }

    /// How the revert is recorded in the undo history.
    pub fn saved_as(self) -> Discarded {
        match self {
            Self::Hunk => Discarded::RevertedHunk,
            Self::Lines(n) => Discarded::RevertedLines(n),
            Self::File => Discarded::RevertedFile,
        }
    }
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
            | Self::RevertLines
            | Self::RevertFile(..)
            | Self::LocalOpDone(Ok(None))
            | Self::Reverted { result: Ok(Reverted::Applied | Reverted::Removed | Reverted::Merged), undo: None, .. } => None,
            Self::LocalOpDone(Err(e)) => Some(Inactive::Log(e)),
            // A discard in a tab switched away from: its undo belongs to that tab.
            Self::LocalOpDone(Ok(Some(action))) => Some(Inactive::Undo(action)),
            // A revert in a tab switched away from: its undo belongs to that tab too.
            Self::Reverted { result: Ok(Reverted::Applied | Reverted::Removed | Reverted::Merged), undo: Some(a), .. } => {
                Some(Inactive::Undo(a))
            }
            Self::Reverted { path, what, result: Ok(r @ Reverted::Conflicts), undo } => {
                let text = crate::update::revert::done_text(&path, what, r);
                Some(match undo {
                    Some(undo) => Inactive::LogAndUndo { text, undo },
                    None => Inactive::Log(text),
                })
            }
            Self::Reverted { path, what, result: Err(e), .. } => Some(Inactive::Log(crate::update::revert::fail_text(&path, what, &e))),
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
    /// The merge banner's Commit merge: fills a blank draft from MERGE_MSG, then commits.
    CommitMerge,
}

impl CommitMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Commit
            | Self::StageAllAndCommit
            | Self::SetPushAfterCommit(..)
            | Self::PrefillAmend
            | Self::AppendToDescription(..)
            | Self::CommitMerge => None,
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
    /// The journal's Restore: `cmd` as a `Git` job that writes the journal note `op`, run only
    /// while `expect`'s revision still resolves to its commit.
    Journaled {
        label: String,
        cmd: Vec<String>,
        op: trellis_core::journal::Op,
        expect: Option<(String, Oid)>,
    },
    /// The in-progress banner's confirmed Abort (`args`, named `label`). With `keep`, the
    /// resolved changes are stashed first so Undo brings them back (`state::op_confirm`).
    Abort {
        label: String,
        args: Vec<String>,
        keep: bool,
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
            | Self::Journaled { .. }
            | Self::Abort { .. }
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
