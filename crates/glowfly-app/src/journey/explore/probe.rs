//! Probes: whether a step's action took effect, read from the repository on disk before and
//! after it. A rule that finds a step failed without saying so builds on this.

use super::action::Action;
use git2::{Oid, Repository, RepositoryState, StatusOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What the probes and rules read from a repository, before and after a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Reading {
    pub head: Option<Oid>,
    /// The first line of the head commit's message, and its parents.
    pub head_summary: Option<String>,
    pub head_parents: Vec<Oid>,
    pub branch: Option<String>,
    pub branches: BTreeMap<String, Oid>,
    pub stashes: usize,
    pub state: RepositoryState,
    pub conflicted: BTreeSet<String>,
    /// Repository-relative paths (with `/`) with staged, unstaged, untracked or conflicted changes.
    pub changed: BTreeSet<String>,
    pub staged: BTreeSet<String>,
}

/// Reads `repo` (a work tree path).
pub(super) fn read(repo: &Path) -> Reading {
    let r = Repository::open(repo).unwrap_or_else(|e| panic!("{} is not a repository: {e}", repo.display()));
    let head_ref = r.head().ok();
    let head = head_ref.as_ref().and_then(|h| h.target());
    let branch = head_ref.as_ref().filter(|h| h.is_branch()).map(|h| h.shorthand().unwrap_or_default().to_owned());
    let commit = head.and_then(|id| r.find_commit(id).ok());
    let head_summary = commit.as_ref().map(|c| {
        let message = String::from_utf8_lossy(c.message_bytes()).into_owned();
        message.lines().next().unwrap_or_default().trim_end().to_owned()
    });
    let head_parents = commit.as_ref().map(|c| c.parent_ids().collect()).unwrap_or_default();
    let mut branches = BTreeMap::new();
    for b in r.branches(Some(git2::BranchType::Local)).into_iter().flatten().flatten() {
        let (b, _) = b;
        if let (Ok(Some(name)), Some(id)) = (b.name(), b.get().target()) {
            branches.insert(name.to_owned(), id);
        }
    }
    let stashes = r.reflog("refs/stash").map(|l| l.len()).unwrap_or(0);
    let conflicted: BTreeSet<String> = r
        .index()
        .and_then(|i| i.conflicts().map(|c| c.flatten().collect::<Vec<_>>()))
        .map(|all| {
            all.into_iter()
                .filter_map(|c| {
                    let e = c.our.or(c.their).or(c.ancestor)?;
                    Some(String::from_utf8_lossy(&e.path).into_owned())
                })
                .collect()
        })
        .unwrap_or_default();
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false);
    let mut changed = conflicted.clone();
    let mut staged = BTreeSet::new();
    if let Ok(statuses) = r.statuses(Some(&mut opts)) {
        for s in statuses.iter() {
            let Some(path) = s.path() else { continue };
            changed.insert(path.to_owned());
            if s.status().intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE,
            ) {
                staged.insert(path.to_owned());
            }
        }
    }
    Reading { head, head_summary, head_parents, branch, branches, stashes, state: r.state(), conflicted, changed, staged }
}

fn mid_merge_or_rebase(r: &Reading) -> bool {
    use RepositoryState::*;
    matches!(r.state, Merge | Rebase | RebaseInteractive | RebaseMerge) && !r.conflicted.is_empty()
}

/// Whether `action` took effect, from the readings around it. None for actions with nothing to
/// probe (SelectWip, Refresh, SwitchTab, Confirm "Cancel", and what the explorer did itself).
///
/// Continue can only be told from the readings as: the repository is clean, or it is still in a
/// rebase and the head moved or the conflicted set changed (the next step's conflict is shown).
pub(super) fn took_effect(action: &Action, before: &Reading, after: &Reading) -> Option<bool> {
    use Action::*;
    let head_moved = before.head != after.head;
    Some(match action {
        SelectWip | Refresh | SwitchTab(_) | CloseTab(_) | StageHunk(_) | Confirm(_) | Outside(_) => return None,
        UnstageAll if before.staged.is_empty() => return None,
        UnstageAll => after.staged.is_empty(),
        Commit(text) => head_moved && after.head_summary.as_deref() == Some(text.as_str()),
        Amend => head_moved && before.head_parents == after.head_parents,
        CreateBranch(n) => after.branches.contains_key(n),
        Checkout(n) => after.branch.as_deref() == Some(n.as_str()),
        DeleteBranch(n) => !after.branches.contains_key(n),
        Merge(_) | Rebase(_) => head_moved || (!mid_merge_or_rebase(before) && mid_merge_or_rebase(after)),
        Stash => after.stashes == before.stashes + 1,
        Unstash => {
            after.stashes + 1 == before.stashes || (after.stashes == before.stashes && !after.conflicted.is_subset(&before.conflicted))
        }
        DropStash => after.stashes + 1 == before.stashes,
        Stage(p) => after.staged.contains(p),
        Unstage(p) => !after.staged.contains(p),
        StageAll => after.staged == after.changed,
        Discard(p) => !after.changed.contains(p),
        ResolveOurs(p) | ResolveTheirs(p) => !after.conflicted.contains(p),
        Continue => {
            (before.state != RepositoryState::Clean && after.state == RepositoryState::Clean)
                || (matches!(after.state, RepositoryState::Rebase | RepositoryState::RebaseInteractive | RepositoryState::RebaseMerge)
                    && (head_moved || before.conflicted != after.conflicted))
        }
        Abort => after.state == RepositoryState::Clean,
        Undo | Redo => {
            head_moved
                || before.branches != after.branches
                || before.stashes != after.stashes
                || before.staged != after.staged
                || before.changed != after.changed
                || before.conflicted != after.conflicted
        }
    })
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
