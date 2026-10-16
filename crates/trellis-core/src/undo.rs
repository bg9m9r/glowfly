//! Undo and redo for Trellis's own git operations. `before`/`after` record the repository state
//! around an operation and turn it into an `Action`; `plan` checks that the repository still
//! matches it and returns the git commands that reverse (or re-apply) it. Reads use libgit2; the
//! planned commands are meant to run through `gitcli`, so hooks and user config apply.
//!
//! `kinds` recognises the command lines, `action` names what was recorded, `record` captures the
//! state around an operation, and `planning` decides how to reverse it, with `refs` (branches and
//! tags), `moved` (merge, pull, cherry-pick, revert, rebase), `stash`, `reset` and `saved`.

use crate::gitcli::args;
use git2::Oid;

mod action;
mod kinds;
mod moved;
mod planning;
mod record;
mod refs;
mod reset;
mod saved;
mod stash;
pub use action::{Action, Changes, Created, HeadPos};
pub use kinds::{MoveOp, ResetMode, UndoKind};
use planning::{has_staged, is_clean, stale};
pub use planning::{plan, plan_with};
use record::stash_top;
pub use record::{Before, after, before, irreversible, redone};
pub use reset::{HARD_RESET_STASH_MESSAGE, hard_reset_commands};
pub use saved::{RestoreFailed, Saved, discard_saving, restore_saving, write as write_saved};

/// The message of the stash that "Discard all" moves uncommitted changes into.
pub const DISCARD_STASH_MESSAGE: &str = "Trellis: discarded changes";

/// The message of the stash "Stash changes and undo" moves local changes into first.
pub const UNDO_STASH_MESSAGE: &str = "Trellis: changes before undo";

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

/// The command Trellis runs for "Discard all".
pub fn discard_command() -> Vec<String> {
    args(&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE])
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

#[cfg(test)]
mod kinds_tests;
#[cfg(test)]
mod moved_tests;
#[cfg(test)]
mod refs_tests;
#[cfg(test)]
mod reset_tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod saved_tests;
#[cfg(test)]
mod stash_tests;
#[cfg(test)]
mod tests;
