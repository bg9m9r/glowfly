//! Undo and redo for Glowfly's own git operations. `before`/`after` record the repository state
//! around an operation and turn it into an `Action`; `plan` checks that the repository still
//! matches it and returns the git commands that reverse (or re-apply) it. Reads use libgit2; the
//! planned commands are meant to run through `gitcli`, so hooks and user config apply.
//!
//! `kinds` recognises the command lines, `action` names what was recorded, `record` captures the
//! state around an operation, and `planning` decides how to reverse it, with `refs` (branches and
//! tags, also a branch the journal's Restore moved with `update-ref`), `moved` (merge, pull,
//! cherry-pick, revert, rebase), `stash`, `reset`, `saved` and `deleted` (Delete and Unstage and
//! delete).
//! Rebases, also interactive ones and graph edits, are recorded around their run instead
//! (`rebased.rs`): they finish in steps, possibly outside Glowfly. A merge, cherry-pick or revert
//! that stopped for conflicts and was finished later (Continue, Skip or a commit) is recorded as
//! one move from the HEAD it started at (`resumed.rs`). `resolved` handles conflict resolutions
//! saved in the conflict view: the conflict's stages come back from git's resolve-undo record.
//! A restack, which moves several branches at once, is recorded around its run too and undone
//! branch by branch (`restacked.rs`). An abort of a merge, cherry-pick or revert keeps its work
//! (resolutions, conflicts and the operation's state) as git objects first and is undone by
//! putting all of it back (`aborted.rs`, `aborted_restore.rs`).

use crate::gitcli::args;
use crate::repo::StagedNew;
use git2::Oid;

mod aborted;
mod aborted_restore;
mod aborted_run;
mod action;
mod deleted;
mod kinds;
mod moved;
mod planning;
mod rebased;
mod record;
mod refs;
mod reset;
mod resolved;
mod restacked;
mod resumed;
mod reverted;
mod saved;
mod stash;
mod unstaged;
pub use aborted::ABORT_REF_PREFIX;
pub use aborted_restore::restore_abort;
pub use aborted_run::{AbortRun, NOTHING_ABORTED, abort_keeping};
pub use action::{Action, BranchMove, Changes, Created, Discarded, HeadPos, Staged};
pub use deleted::{
    DeleteFailed, DeleteVia, DeletedSaving, MAX_KEPT_BYTES, PutBackHow, Trashers, delete_again, delete_saving, delete_staged_saving,
    discard_untracked_saving, put_back,
};
pub use kinds::{MoveOp, ResetMode, UndoKind};
pub(crate) use kinds::{commits_label, move_targets};
use planning::{has_staged, is_clean, stale};
pub use planning::{plan, plan_with};
pub use rebased::{RebaseStart, rebase_start, rebased, unmoved};
pub(crate) use record::head;
use record::stash_top;
pub use record::{Before, after, before, irreversible, redone};
pub use reset::{HARD_RESET_STASH_MESSAGE, hard_reset_commands};
pub use resolved::resolve_saving;
pub use restacked::{RestackBefore, restack_before, restack_unmoved, restacked};
pub use reverted::{RevertFailed, revert_saving};
pub use saved::{RestoreFailed, Saved, discard_saving, restore_saving, write as write_saved};
pub use unstaged::{restage, unstage_again, unstage_gone_saving};

/// The message of the stash that "Discard all" moves uncommitted changes into.
pub const DISCARD_STASH_MESSAGE: &str = "Glowfly: discarded changes";

/// The message of the stash "Stash changes and undo" moves local changes into first.
pub const UNDO_STASH_MESSAGE: &str = "Glowfly: changes before undo";

/// Why an undo or redo can't run now.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// A merge, rebase or similar is in progress.
    Busy(String),
    /// Running it would overwrite uncommitted work; the entry stays usable.
    Dirty(String),
    /// The repository no longer matches the history, which should be cleared.
    Stale(String),
    /// Running it would lose commits or needs the user to act first; the entry stays.
    Unsafe(String),
}

impl Refusal {
    pub fn message(&self) -> &str {
        match self {
            Refusal::Busy(m) | Refusal::Dirty(m) | Refusal::Stale(m) | Refusal::Unsafe(m) => m,
        }
    }

    pub fn is_stale(&self) -> bool {
        matches!(self, Refusal::Stale(_))
    }
}

#[derive(Debug, PartialEq)]
pub enum Plan {
    /// Git commands to run in order.
    Run(Vec<Vec<String>>),
    /// Make the working-tree file `path` hold `content` exactly (`None`: delete it).
    Write {
        path: String,
        content: Option<Saved>,
    },
    Refuse(Refusal),
    /// Run `cmds` in order, then make the working-tree file `path` hold `content` exactly
    /// (`None`: delete it).
    RunThenWrite {
        cmds: Vec<Vec<String>>,
        path: String,
        content: Option<Saved>,
    },
    /// Put the deleted file `path` back (it must be absent): out of the trash when it went there
    /// `via` one, else written as `content`; and its `staged` entry back in the index.
    PutBack {
        path: String,
        content: Saved,
        staged: Option<StagedNew>,
        via: DeleteVia,
    },
    /// Put `staged` back in the index at `path` (undo of Unstage deleted file).
    Restage {
        path: String,
        staged: StagedNew,
    },
    /// Take `path`, staged as new and gone from the working tree, out of the index again.
    UnstageAgain {
        path: String,
    },
    /// Delete `path` again via `via`, unstaging it too when it was `staged` as new.
    DeleteAgain {
        path: String,
        via: DeleteVia,
        staged: bool,
    },
    /// Put a kept abort back (`restore_abort`): the operation is in progress again.
    RestoreAbort,
    /// Abort the operation in progress again, keeping its work (`abort_keeping`).
    AbortAgain,
}

/// Choices the user made for one undo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    /// Undoing a checkout that created a branch also deletes the branch.
    pub delete_created: bool,
}

/// The command that stashes local changes (untracked files too) before "Stash changes and undo".
pub fn stash_first_command() -> Vec<String> {
    args(&["stash", "push", "-u", "-m", UNDO_STASH_MESSAGE])
}

/// The command Glowfly runs for "Discard all".
pub fn discard_command() -> Vec<String> {
    args(&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE])
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

#[cfg(test)]
mod aborted_safety_tests;
#[cfg(test)]
mod aborted_tests;
#[cfg(test)]
mod deleted_tests;
#[cfg(test)]
mod kinds_tests;
#[cfg(test)]
mod moved_tests;
#[cfg(test)]
mod rebased_tests;
#[cfg(test)]
mod refs_tests;
#[cfg(test)]
mod reset_tests;
#[cfg(test)]
mod resolved_tests;
#[cfg(test)]
mod restacked_tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod resumed_tests;
#[cfg(test)]
mod reverted_tests;
#[cfg(test)]
mod saved_tests;
#[cfg(test)]
mod stash_tests;
#[cfg(test)]
mod test_bin;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_file_actions_tests;
