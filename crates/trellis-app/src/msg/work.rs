//! Messages for staging, committing and running git jobs (update/ops.rs, update/commit.rs,
//! update/undo.rs, update/stash.rs).

use crate::effect::UndoResult;
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
    /// Undo the active tab's most recent undoable operation.
    Undo,
    Redo,
    JobDone {
        ok: bool,
        output: String,
        undo: Option<UndoResult>,
    },
}
