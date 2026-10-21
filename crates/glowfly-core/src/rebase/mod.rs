//! Interactive rebase: the commits a rebase would replay (`plan`), the todo list Glowfly
//! writes for git in place of the one git generates (`todo`), running and resuming it (`run`),
//! history edits chosen in the graph (`quick`), where a rebase in progress stands
//! (`progress`), and the steps a running rebase has left, edited with `git rebase --edit-todo`
//! (`remaining`). A range that holds merge commits gets a todo that recreates them (`merges`):
//! each line of history starts with a `reset`, merges are redone with `merge -C`, and `label`
//! steps name the commits later lines start from or merge.
//!
//! Git is started with `GIT_SEQUENCE_EDITOR` set to a command that replaces git's todo with
//! the prepared one (Glowfly itself in `--write-todo` mode, after checking that both list the
//! same commits). Rewording is done by `exec git commit --amend -F <file>` lines, so no
//! interactive editor ever opens. The list can also hold `exec` and `break` steps, and a
//! typed message for the commit a squash group combines into.

use git2::{Oid, RepositoryState};
use std::path::PathBuf;

mod merges;
mod plan;
mod progress;
mod quick;
mod remaining;
mod run;
mod todo;

pub use plan::plan;
pub(crate) use progress::progress_of;
pub use progress::{RebaseProgress, progress, stopped_at};
pub use quick::{QuickEdit, QuickPlan, prepare_edit};
pub use remaining::{EditTodoRequest, Remaining, edit_todo, remaining};
pub use run::{Resume, resume, start};
pub(crate) use run::{outcome, stash_tip};
pub use todo::{
    build, default_combined, editor_command, group_end, install_todo, is_unchanged, listed_oids, problem, problem_with, shell_quote,
    shell_quote_for,
};

/// Why Start refuses a list made before HEAD moved.
pub const STALE: &str = "The branch changed since this list was made. Reset to reload it.";
/// Why a todo is refused when git would rebase different commits than the list holds.
pub const CHANGED: &str = "The commits to rebase changed; reload the list and try again.";
pub const DETACHED: &str = "Check out a branch to rebase it.";
pub const FIRST_COMBINES: &str =
    "The first commit can't be squashed or fixed up: there is no earlier commit in the list to combine it with.";
pub const EMPTY_REWORD: &str = "Reword messages can't be empty.";
pub const NOT_ON_BRANCH: &str = "This commit isn't on the current branch.";
pub const SOME_NOT_ON_BRANCH: &str = "Some selected commits aren't on the current branch.";
pub const MERGE_SELECTED: &str = "Merge commits can't be edited this way.";
pub const SOME_MERGE_SELECTED: &str = "The selection includes a merge commit.";
/// A commit reachable from HEAD only through a merge's later parents.
pub const SIDE_LINE: &str = "This commit came in through a merge; edit it in the interactive rebase panel instead.";
pub const SOME_SIDE_LINE: &str = "Some selected commits came in through a merge; edit them in the interactive rebase panel instead.";
/// A squash whose commits follow one another on the line, but the older one starts another line.
pub const SPLIT_BY_BRANCH: &str = "Another branch starts between these commits; squash them in the interactive rebase panel instead.";
pub const COMBINE_AFTER_MERGE: &str = "A squash or fixup must follow the commit it melds into, not a merge, label or reset step.";
/// A commit row a later `reset` would throw away (see `merges::loses_commit`).
pub const LOST_TO_RESET: &str =
    "A commit just before a reset step would be left out of the branch. Move it below the reset, or above the label that ends its line.";
pub const ROOT_SQUASH: &str = "The first commit has no parent to squash it into.";
pub const PARENT_MERGE: &str = "Its parent is a merge commit, which can't be squashed into.";
pub const NOT_CONSECUTIVE: &str = "Select commits that follow one another to squash them.";
pub const TOO_FEW_TO_SQUASH: &str = "Select two or more commits to squash them.";
pub const DROPS_EVERYTHING: &str = "Dropping every commit of the branch isn't supported.";
pub const NOT_REBASING: &str = "No rebase is in progress.";
pub const EMPTY_COMBINED: &str = "The combined commit's message can't be empty.";
pub const EMPTY_EXEC: &str = "Exec steps need a command.";
pub const EXEC_ONE_LINE: &str = "An exec command must be one line.";
pub const COMBINE_AFTER_STEP: &str = "Move the exec or break step below the squash or fixup: it must follow the commit it combines with.";
pub const NOT_INTERACTIVE: &str = "Only an interactive rebase's remaining steps can be edited.";
pub const UNSUPPORTED_STEPS: &str = "The remaining steps include merge, label, reset or update-ref commands, which this panel can't edit yet. Use git rebase --edit-todo in a terminal.";
pub const TODO_MOVED: &str = "The rebase moved on since this list was read. Reset to reload it.";

/// Why a rebase onto `oid` would do nothing: `branch` already contains it.
pub fn contains_commit(branch: &str, oid: Oid) -> String {
    format!("{branch} already contains {}.", short(oid))
}

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
    /// What a merge step can do.
    pub const MERGE_ACTIONS: [Self; 3] = [Self::Pick, Self::Reword, Self::Drop];

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
    /// The message typed for the combined commit when this row heads a squash group; `None`
    /// keeps the one git makes.
    pub combined_message: Option<String>,
    pub author: String,
    pub time: i64,
}

impl TodoItem {
    /// A reword that actually changes the message.
    pub fn rewords(&self) -> bool {
        self.action == RebaseAction::Reword && self.new_message.as_ref().is_some_and(|m| m.trim() != self.message.trim())
    }
}

/// Where a line of history of a merge-preserving todo starts (`reset`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ResetTo {
    /// The new base.
    Onto,
    /// A commit rewritten earlier, by its label.
    Label(String),
    /// A commit outside the range, kept where it is.
    Commit(Oid),
    /// A new first commit (`reset [new root]`).
    NewRoot,
}

/// A later parent of a recreated merge.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MergeParent {
    /// A commit of the range, rewritten earlier, by its label.
    Label(String),
    /// A commit outside the range, kept as it is.
    Commit(Oid),
}

/// A merge commit redone with `merge -C`: the merge (`item.action` is Pick, Reword or Drop)
/// and its parents after the first, which is wherever the line stands.
#[derive(Clone, PartialEq, Debug)]
pub struct MergeStep {
    pub item: TodoItem,
    pub parents: Vec<MergeParent>,
}

/// One step of the todo list: a commit, a shell command (`exec`), a pause (`break`), or, in a
/// list that recreates merges, a label, a reset or a merge.
#[derive(Clone, PartialEq, Debug)]
pub enum TodoStep {
    Commit(TodoItem),
    Exec(String),
    Break,
    /// Names HEAD (the rewritten `at`) for a later reset or merge.
    Label {
        name: String,
        at: Oid,
    },
    Reset(ResetTo),
    Merge(MergeStep),
}

impl TodoStep {
    /// A commit row (never a merge, so a merge never heads a squash group).
    pub fn commit(&self) -> Option<&TodoItem> {
        match self {
            Self::Commit(i) => Some(i),
            _ => None,
        }
    }

    pub fn commit_mut(&mut self) -> Option<&mut TodoItem> {
        match self {
            Self::Commit(i) => Some(i),
            _ => None,
        }
    }

    pub fn merge(&self) -> Option<&MergeStep> {
        match self {
            Self::Merge(m) => Some(m),
            _ => None,
        }
    }

    /// The commit of a commit or merge row.
    pub fn item(&self) -> Option<&TodoItem> {
        match self {
            Self::Commit(i) | Self::Merge(MergeStep { item: i, .. }) => Some(i),
            _ => None,
        }
    }

    pub fn item_mut(&mut self) -> Option<&mut TodoItem> {
        match self {
            Self::Commit(i) | Self::Merge(MergeStep { item: i, .. }) => Some(i),
            _ => None,
        }
    }

    /// The commit id of a commit or merge row.
    pub fn oid(&self) -> Option<Oid> {
        self.item().map(|i| i.oid)
    }

    /// A label, reset or merge step: part of the shape of a merge-preserving list, fixed in
    /// place.
    pub fn is_structure(&self) -> bool {
        matches!(self, Self::Label { .. } | Self::Reset(_) | Self::Merge(_))
    }
}

/// Where an interactive rebase starts from.
#[derive(Clone, PartialEq, Debug)]
pub enum RebaseBase {
    /// This commit and every later one on the current branch.
    FromCommit(Oid),
    /// The current branch's commits that aren't on this local branch, onto its tip.
    OntoBranch(String),
    /// The current branch's commits that aren't reachable from this commit, onto it.
    OntoCommit(Oid),
}

/// What `git rebase -i` is given: a commit, or `--root`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Upstream {
    Commit(Oid),
    Root,
}

/// The steps to run, oldest first, and where they go.
#[derive(Clone, Debug)]
pub struct RebasePlan {
    pub head: Oid,
    pub branch: String,
    pub upstream: Upstream,
    /// "main", "1a2b3c4 Initial commit", or empty for `Root`.
    pub onto_label: String,
    /// All picks; label, reset and merge steps too when the range holds merges.
    pub steps: Vec<TodoStep>,
}

/// The list to run, with the HEAD it was made for.
#[derive(Clone, Debug)]
pub struct StartRequest {
    pub head: Oid,
    pub upstream: Upstream,
    pub items: Vec<TodoStep>,
    /// Stash tracked changes first and put them back when the rebase ends (`--autostash`).
    pub autostash: bool,
}

/// The command git runs as `GIT_SEQUENCE_EDITOR` (git appends the todo file's path).
#[derive(Debug)]
pub enum SequenceEditor {
    /// Glowfly's own executable in `--write-todo` mode: checks, then installs the todo.
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
    /// Glowfly's reword amend failed.
    Exec,
    /// An `exec` step the user added failed.
    Command,
    /// Stopped at a `break` step.
    Break,
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
    /// No rebase is left in progress. `stash_kept`: the autostash didn't apply cleanly, so git
    /// kept it in the stash list.
    Finished { output: String, stash_kept: bool },
    /// A rebase is in progress (at an edit step, a conflict or a failed reword); `ok` is git's
    /// exit status.
    Stopped { ok: bool, output: String, progress: RebaseProgress },
    /// Git failed and left no rebase in progress.
    Failed { output: String },
}

/// Why nothing can start while an operation is in progress, if one is.
pub fn in_progress_text(state: RepositoryState) -> Option<String> {
    let op = operation_noun(state)?;
    Some(format!("Finish or abort the {op} in progress before starting an interactive rebase."))
}

/// The operation in progress ("merge", "rebase", "cherry-pick", "revert", "patch application"
/// or "bisect"), if any.
pub fn operation_noun(state: RepositoryState) -> Option<&'static str> {
    use RepositoryState::*;
    Some(match state {
        Clean => return None,
        Merge => "merge",
        Rebase | RebaseInteractive | RebaseMerge => "rebase",
        CherryPick | CherryPickSequence => "cherry-pick",
        Revert | RevertSequence => "revert",
        ApplyMailbox | ApplyMailboxOrRebase => "patch application",
        Bisect => "bisect",
    })
}

/// The first seven characters of a commit id.
pub(crate) fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

#[cfg(test)]
mod merges_tests;
#[cfg(test)]
mod merges_todo_tests;
#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod progress_tests;
#[cfg(test)]
mod quick_tests;
#[cfg(test)]
mod remaining_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod todo_tests;
