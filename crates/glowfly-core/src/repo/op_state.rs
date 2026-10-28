//! The operation in progress as Glowfly shows it: libgit2's state, except that a cherry-pick or
//! revert sequence whose stopped commit was committed (git removed CHERRY_PICK_HEAD or
//! REVERT_HEAD, so libgit2 says clean) is still a sequence while `sequencer/todo` holds a pick
//! or revert, so its banner and Continue stay up.

use git2::{Repository, RepositoryState};

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
