//! The operation in progress as Glowfly shows it: libgit2's state, except that a cherry-pick or
//! revert sequence whose stopped commit was committed (git removed CHERRY_PICK_HEAD or
//! REVERT_HEAD, so libgit2 says clean) is still a sequence while `sequencer/todo` holds a pick
//! or revert, so its banner and Continue stay up. Also the branch a worktree is rebasing or
//! bisecting while its HEAD is detached (`held_branch`), read from the same state files, so it
//! counts as checked out there.

use git2::{Repository, RepositoryState};
use std::path::Path;

/// `repo.state()`, or the pick or revert sequence `sequencer/todo` still holds when that says
/// clean.
pub fn state(repo: &Repository) -> RepositoryState {
    match repo.state() {
        RepositoryState::Clean => sequence_left(repo).unwrap_or(RepositoryState::Clean),
        s => s,
    }
}

/// The sequence the first line of `sequencer/todo` that isn't a comment belongs to; None
/// without a todo, or when that line is neither a pick nor a revert.
fn sequence_left(repo: &Repository) -> Option<RepositoryState> {
    let todo = std::fs::read_to_string(repo.path().join("sequencer").join("todo")).ok()?;
    let line = todo.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
    let word = line.split_whitespace().next()?;
    match word {
        "pick" | "p" => Some(RepositoryState::CherryPickSequence),
        "revert" => Some(RepositoryState::RevertSequence),
        _ => None,
    }
}

/// Whether `state` is one whose commit message git prepared in MERGE_MSG for the commit box.
pub(super) fn has_merge_msg(state: RepositoryState) -> bool {
    use RepositoryState::*;
    matches!(state, Merge | CherryPick | CherryPickSequence | Revert | RevertSequence)
}

/// How a worktree has a local branch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BranchUse {
    /// HEAD names it.
    CheckedOut,
    /// A rebase of it stopped there; HEAD is detached until it ends.
    Rebasing,
    /// A bisect started from it runs there; HEAD is detached until `git bisect reset`.
    Bisecting,
}

/// The local branch a worktree is rebasing or bisecting (`by`: Rebasing or Bisecting).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct HeldBranch {
    /// Short name (without refs/heads/).
    pub name: String,
    pub by: BranchUse,
}

/// The branch the worktree whose own git dir is `git_dir` (the main worktree's is the common
/// dir) is rebasing or bisecting, read as git does to refuse it elsewhere: a rebase's
/// `rebase-merge/head-name` or `rebase-apply/head-name` (`refs/heads/<b>`, or `detached HEAD`),
/// else, while a bisect runs (`BISECT_LOG`, the file libgit2's state reads too), the branch
/// `BISECT_START` names (a commit there, from a bisect started detached, names none).
pub fn held_branch(git_dir: &Path) -> Option<HeldBranch> {
    let read = |p: &str| std::fs::read_to_string(git_dir.join(p)).ok();
    for dir in ["rebase-merge", "rebase-apply"] {
        if let Some(text) = read(&format!("{dir}/head-name")) {
            let name = text.trim().strip_prefix("refs/heads/")?;
            return (!name.is_empty()).then(|| HeldBranch { name: name.to_owned(), by: BranchUse::Rebasing });
        }
    }
    if !git_dir.join("BISECT_LOG").is_file() {
        return None;
    }
    let start = read("BISECT_START")?;
    let start = start.trim();
    let name = start.strip_prefix("refs/heads/").unwrap_or(start);
    let commit = matches!(name.len(), 40 | 64) && name.bytes().all(|b| b.is_ascii_hexdigit());
    (!name.is_empty() && !commit).then(|| HeldBranch { name: name.to_owned(), by: BranchUse::Bisecting })
}
