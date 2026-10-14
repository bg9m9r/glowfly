//! Interactive rebase: the commits a rebase would replay (`plan`), the todo list Trellis
//! writes for git in place of the one git generates (`todo`), and running it (`run`).
//!
//! Git is started with `GIT_SEQUENCE_EDITOR` set to a command that replaces git's todo with
//! the prepared one (Trellis itself in `--write-todo` mode, after checking that both list the
//! same commits). Rewording is done by `exec git commit --amend -F <file>` lines, so no
//! interactive editor ever opens.

use git2::{Oid, RepositoryState};
use std::path::PathBuf;

mod plan;
mod run;
mod todo;

pub use plan::plan;
pub use run::{start, stopped_at};
pub use todo::{build, editor_command, install_todo, is_unchanged, listed_oids, problem, shell_quote};

/// Why Start refuses a list made before HEAD moved.
pub const STALE: &str = "The branch changed since this list was made. Reset to reload it.";
/// Why a todo is refused when git would rebase different commits than the list holds.
pub const CHANGED: &str = "The commits to rebase changed; reload the list and try again.";
pub const DETACHED: &str = "Check out a branch to rebase it.";
pub const FIRST_COMBINES: &str =
    "The first commit can't be squashed or fixed up: there is no earlier commit in the list to combine it with.";
pub const EMPTY_REWORD: &str = "Reword messages can't be empty.";

/// What git does with one commit of the todo list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RebaseAction {
    Pick,
    Reword,
    Squash,
    Fixup,
    Drop,
    Edit,
}

impl RebaseAction {
    pub const ALL: [Self; 6] = [Self::Pick, Self::Reword, Self::Squash, Self::Fixup, Self::Drop, Self::Edit];

    /// The todo-list command.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Reword => "reword",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
            Self::Edit => "edit",
        }
    }

    /// Melds the commit into the one before it.
    pub fn combines(self) -> bool {
        matches!(self, Self::Squash | Self::Fixup)
    }
}

/// One commit of the list, with what to do with it.
#[derive(Clone, PartialEq, Debug)]
pub struct TodoItem {
    pub oid: Oid,
    pub action: RebaseAction,
    pub summary: String,
    /// The full original message.
    pub message: String,
    /// The message typed for a reword; `None` keeps the original.
    pub new_message: Option<String>,
    pub author: String,
    pub time: i64,
}

impl TodoItem {
    /// A reword that actually changes the message.
    pub fn rewords(&self) -> bool {
        self.action == RebaseAction::Reword && self.new_message.as_ref().is_some_and(|m| m.trim() != self.message.trim())
    }
}

/// Where an interactive rebase starts from.
#[derive(Clone, PartialEq, Debug)]
pub enum RebaseBase {
    /// This commit and every later one on the current branch.
    FromCommit(Oid),
    /// The current branch's commits that aren't on this local branch, onto its tip.
    OntoBranch(String),
}

/// What `git rebase -i` is given: a commit, or `--root`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Upstream {
    Commit(Oid),
    Root,
}

/// The commits to rebase, oldest first, and where they go.
#[derive(Clone, Debug)]
pub struct RebasePlan {
    pub head: Oid,
    pub branch: String,
    pub upstream: Upstream,
    /// "main", "1a2b3c4 Initial commit", or empty for `Root`.
    pub onto_label: String,
    pub items: Vec<TodoItem>,
}

/// The list to run, with the HEAD it was made for.
#[derive(Debug)]
pub struct StartRequest {
    pub head: Oid,
    pub upstream: Upstream,
    pub items: Vec<TodoItem>,
}

/// The command git runs as `GIT_SEQUENCE_EDITOR` (git appends the todo file's path).
#[derive(Debug)]
pub enum SequenceEditor {
    /// Trellis's own executable in `--write-todo` mode: checks, then installs the todo.
    Helper(PathBuf),
    /// `cp`: installs the todo without checking (tests).
    Copy,
    /// A raw command (tests, e.g. a failing editor).
    Shell(String),
}

/// Why a rebase stopped.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StopKind {
    /// At an `edit` step.
    Edit,
    /// A pick or squash conflicted.
    Conflict,
    /// An `exec` line (a reword's amend) failed.
    Exec,
    Other,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Stop {
    pub kind: StopKind,
    /// The commit it stopped at, when known.
    pub oid: Option<Oid>,
}

#[derive(Debug)]
pub enum Outcome {
    Finished {
        output: String,
    },
    /// A rebase is in progress (at an edit step, a conflict or a failed reword); `ok` is git's
    /// exit status.
    Stopped {
        ok: bool,
        output: String,
        stop: Stop,
    },
    /// Git failed and left no rebase in progress.
    Failed {
        output: String,
    },
}

/// Why nothing can start while an operation is in progress, if one is.
pub fn in_progress_text(state: RepositoryState) -> Option<String> {
    use RepositoryState::*;
    let op = match state {
        Clean => return None,
        Merge => "merge",
        Rebase | RebaseInteractive | RebaseMerge => "rebase",
        CherryPick | CherryPickSequence => "cherry-pick",
        Revert | RevertSequence => "revert",
        ApplyMailbox | ApplyMailboxOrRebase => "patch application",
        Bisect => "bisect",
    };
    Some(format!("Finish or abort the {op} in progress before starting an interactive rebase."))
}

/// The first seven characters of a commit id.
pub(crate) fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod todo_tests;
