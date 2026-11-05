//! Why a rebase, a graph history edit, a remaining-steps edit or a restack was refused or
//! failed (`RebaseError`, #367): Glowfly's own reasons as variants the model words in the
//! user's language (state/error_words/typed.rs), git's, libgit2's and the system's own words
//! kept as they came. `Display` gives the English these refusals have always shown, from the
//! sentence constants below (which the panel and tests also name).
//!
//! The `--write-todo` helper runs in its own process, so its refusal crosses over in a side file
//! (`<prepared>.error`) as JSON: [`write_refusal`] writes it and `read_refusal` reads it back,
//! taking text it can't parse (an app updated in place while running) as git's own words.

use super::HeldAlias;
use git2::RepositoryState;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

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
/// A quick drop of a commit another branch points at (`rebase.updateRefs`).
pub const BRANCH_TIP: &str = "Another branch points at this commit and would lose it; drop it in the interactive rebase panel instead.";
pub const SOME_BRANCH_TIPS: &str =
    "Another branch points at a selected commit and would lose it; drop them in the interactive rebase panel instead.";
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
pub const UNSUPPORTED_STEPS: &str = "The remaining steps include merge, label or reset commands, which this panel can't edit yet. Use git rebase --edit-todo in a terminal.";
/// An `update-ref` line between a commit and a squash or fixup into it: the panel keeps a
/// branch's update after the whole group, so it can't write this order back.
pub const UPDATE_REF_IN_GROUP: &str = "The remaining steps move a branch between a commit and a squash or fixup into it, which this panel can't keep as it is. Use git rebase --edit-todo in a terminal.";
/// Why a todo is refused when git would move other branches than the list does.
pub const REFS_CHANGED: &str = "The branches this rebase moves (rebase.updateRefs) changed; reload the list and try again.";
pub const TODO_MOVED: &str = "The rebase moved on since this list was read. Reset to reload it.";
/// A branch name that isn't UTF-8 points at a commit of the range: git would list it, and the
/// list can't name it.
pub const NOT_UTF8: &str =
    "A branch whose name isn't valid UTF-8 points at a commit of this rebase, and git would move it (rebase.updateRefs). Rename it first.";

/// An operation in progress that holds a rebase back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Busy {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    /// `git am` (or an apply that may be a rebase's).
    PatchApplication,
    Bisect,
}

impl Busy {
    /// The operation `state` says is in progress, if any.
    pub fn of(state: RepositoryState) -> Option<Busy> {
        use RepositoryState::*;
        Some(match state {
            Clean => return None,
            Merge => Busy::Merge,
            Rebase | RebaseInteractive | RebaseMerge => Busy::Rebase,
            CherryPick | CherryPickSequence => Busy::CherryPick,
            Revert | RevertSequence => Busy::Revert,
            ApplyMailbox | ApplyMailboxOrRebase => Busy::PatchApplication,
            Bisect => Busy::Bisect,
        })
    }

    /// Its English name: "merge", "rebase", "cherry-pick", "revert", "patch application" or
    /// "bisect".
    pub fn noun(self) -> &'static str {
        match self {
            Busy::Merge => "merge",
            Busy::Rebase => "rebase",
            Busy::CherryPick => "cherry-pick",
            Busy::Revert => "revert",
            Busy::PatchApplication => "patch application",
            Busy::Bisect => "bisect",
        }
    }
}

/// A branch the list moves that git leaves out (`rebase.updateRefs`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeftOut {
    /// The branch, without `refs/heads/`.
    pub name: String,
    /// Where git says it is checked out, as its comment wrote it (quotes included).
    pub checked_out_at: Option<String>,
}

/// Why a rebase, a history edit, a remaining-steps edit or a restack was refused or failed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RebaseError {
    Stale,
    Changed,
    Detached,
    FirstCombines,
    EmptyReword,
    NotOnBranch,
    SomeNotOnBranch,
    MergeSelected,
    SomeMergeSelected,
    SideLine,
    SomeSideLine,
    BranchTip,
    SomeBranchTips,
    SplitByBranch,
    CombineAfterMerge,
    LostToReset,
    RootSquash,
    ParentMerge,
    NotConsecutive,
    TooFewToSquash,
    DropsEverything,
    NotRebasing,
    EmptyCombined,
    EmptyExec,
    ExecOneLine,
    CombineAfterStep,
    NotInteractive,
    UnsupportedSteps,
    UpdateRefInGroup,
    RefsChanged,
    TodoMoved,
    /// [`NOT_UTF8`].
    NotUtf8Branch,
    /// The stack no longer matches the restack's plan (`stack::STALE_PLAN`).
    StackChanged,
    /// Ending a stopped restack while its rebase still runs (`stack::STILL_REBASING`).
    StillRebasing,
    /// HEAD's branch is unborn.
    NoCommitsYet,
    /// Another operation is in progress.
    InProgress {
        op: Busy,
    },
    /// The commit (short id) a rebase from a commit starts at isn't on the current branch.
    CommitNotOnBranch {
        commit: String,
    },
    /// Nothing to rebase: `branch` has no commits that aren't on `onto` (its label).
    NothingToRebase {
        branch: String,
        onto: String,
    },
    /// The local branch to rebase onto doesn't exist.
    BranchNotFound {
        name: String,
    },
    /// A commit (as the plan or the todo names it) can't be found.
    CommitNotFound {
        commit: String,
    },
    /// A branch alias git would move wrongly ([`HeldAlias`]).
    Held {
        alias: String,
        target: String,
        elsewhere: bool,
    },
    /// git leaves out branches the list moves.
    NotMoved {
        branches: Vec<LeftOut>,
    },
    /// `git rebase --edit-todo` failed without saying why.
    EditTodoFailed,
    /// git's, libgit2's or the system's own words.
    Said {
        said: String,
    },
    /// A rebase's runner worker panicked: which one, and the panic's own words.
    Panicked {
        effect: String,
        panic: String,
    },
    /// Any other error, in the words it has always shown.
    Other {
        said: String,
    },
}

impl RebaseError {
    /// The English sentence of a variant without fields.
    fn sentence(&self) -> Option<&'static str> {
        use RebaseError as E;
        Some(match self {
            E::Stale => STALE,
            E::Changed => CHANGED,
            E::Detached => DETACHED,
            E::FirstCombines => FIRST_COMBINES,
            E::EmptyReword => EMPTY_REWORD,
            E::NotOnBranch => NOT_ON_BRANCH,
            E::SomeNotOnBranch => SOME_NOT_ON_BRANCH,
            E::MergeSelected => MERGE_SELECTED,
            E::SomeMergeSelected => SOME_MERGE_SELECTED,
            E::SideLine => SIDE_LINE,
            E::SomeSideLine => SOME_SIDE_LINE,
            E::BranchTip => BRANCH_TIP,
            E::SomeBranchTips => SOME_BRANCH_TIPS,
            E::SplitByBranch => SPLIT_BY_BRANCH,
            E::CombineAfterMerge => COMBINE_AFTER_MERGE,
            E::LostToReset => LOST_TO_RESET,
            E::RootSquash => ROOT_SQUASH,
            E::ParentMerge => PARENT_MERGE,
            E::NotConsecutive => NOT_CONSECUTIVE,
            E::TooFewToSquash => TOO_FEW_TO_SQUASH,
            E::DropsEverything => DROPS_EVERYTHING,
            E::NotRebasing => NOT_REBASING,
            E::EmptyCombined => EMPTY_COMBINED,
            E::EmptyExec => EMPTY_EXEC,
            E::ExecOneLine => EXEC_ONE_LINE,
            E::CombineAfterStep => COMBINE_AFTER_STEP,
            E::NotInteractive => NOT_INTERACTIVE,
            E::UnsupportedSteps => UNSUPPORTED_STEPS,
            E::UpdateRefInGroup => UPDATE_REF_IN_GROUP,
            E::RefsChanged => REFS_CHANGED,
            E::TodoMoved => TODO_MOVED,
            E::NotUtf8Branch => NOT_UTF8,
            E::StackChanged => crate::stack::STALE_PLAN,
            E::StillRebasing => crate::stack::STILL_REBASING,
            E::NoCommitsYet => "The branch has no commits yet.",
            E::EditTodoFailed => "git rebase --edit-todo failed",
            _ => return None,
        })
    }

    /// The branches of `NotMoved` as its English names them: "name" or "name (checked out at
    /// P)", joined with ", ".
    fn left_out(branches: &[LeftOut]) -> String {
        let named: Vec<String> = branches
            .iter()
            .map(|b| match &b.checked_out_at {
                Some(p) => format!("{} (checked out at {p})", b.name),
                None => b.name.clone(),
            })
            .collect();
        named.join(", ")
    }
}

/// The English these refusals have always shown, for tracing, the job output's English and
/// tests; users see `typed::rebase`'s words in their language.
impl fmt::Display for RebaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use RebaseError as E;
        match self {
            E::InProgress { op } => {
                write!(f, "Finish or abort the {} in progress before starting an interactive rebase.", op.noun())
            }
            E::CommitNotOnBranch { commit } => write!(f, "{commit} is not on the current branch"),
            E::NothingToRebase { branch, onto } => write!(f, "{branch} has no commits that aren't already on {onto}"),
            E::BranchNotFound { name } => write!(f, "not found: branch {name}"),
            E::CommitNotFound { commit } => write!(f, "not found: commit {commit}"),
            E::Held { alias, target, elsewhere: true } => write!(
                f,
                "{alias} points at {target}, which is checked out in another worktree; git would move {target} with this rebase (rebase.updateRefs). Delete {alias} or switch that worktree to another branch first."
            ),
            E::Held { alias, target, elsewhere: false } => write!(
                f,
                "{alias} points at {target}, which isn't a local branch; git would move it with this rebase (rebase.updateRefs). Delete {alias} first."
            ),
            E::NotMoved { branches } if branches.iter().any(|b| b.checked_out_at.is_some()) => write!(
                f,
                "git won't move {} with this rebase (rebase.updateRefs), though the list does. Switch that worktree to another branch, or turn rebase.updateRefs off, and try again.",
                Self::left_out(branches)
            ),
            E::NotMoved { branches } => write!(
                f,
                "git won't move {} with this rebase, though the list does: check rebase.updateRefs and the git you run, and try again.",
                Self::left_out(branches)
            ),
            E::Said { said } | E::Other { said } => f.write_str(said),
            E::Panicked { effect, panic } => write!(f, "Glowfly hit an internal error ({effect}): {panic}"),
            unit => f.write_str(unit.sentence().unwrap_or_default()),
        }
    }
}

impl From<crate::error::Error> for RebaseError {
    fn from(e: crate::error::Error) -> Self {
        use crate::error::Error;
        match e {
            Error::Git(e) => RebaseError::Said { said: e.to_string() },
            Error::Io(e) => RebaseError::Said { said: e.to_string() },
            other => RebaseError::Other { said: other.to_string() },
        }
    }
}

impl From<git2::Error> for RebaseError {
    fn from(e: git2::Error) -> Self {
        RebaseError::Said { said: e.to_string() }
    }
}

impl From<std::io::Error> for RebaseError {
    fn from(e: std::io::Error) -> Self {
        RebaseError::Said { said: e.to_string() }
    }
}

impl From<&HeldAlias> for RebaseError {
    fn from(h: &HeldAlias) -> Self {
        RebaseError::Held { alias: h.alias.clone(), target: h.target.clone(), elsewhere: h.elsewhere }
    }
}

/// The side file the `--write-todo` helper leaves its refusal in: `<prepared>.error`.
pub(super) fn refusal_file(prepared: &Path) -> PathBuf {
    let mut name = prepared.as_os_str().to_owned();
    name.push(".error");
    PathBuf::from(name)
}

/// Writes why the `--write-todo` helper refused `prepared`, typed (JSON), to `<prepared>.error`
/// for the app to read back.
pub fn write_refusal(prepared: &Path, e: &RebaseError) -> std::io::Result<()> {
    let json = serde_json::to_string(e).map_err(std::io::Error::other)?;
    std::fs::write(refusal_file(prepared), json)
}

/// The refusal a `<prepared>.error` file holds; text that isn't one (written by another version
/// of the helper) is taken as its own words.
pub fn read_refusal(text: &str) -> RebaseError {
    serde_json::from_str(text.trim()).unwrap_or_else(|_| RebaseError::Said { said: text.trim().to_owned() })
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
