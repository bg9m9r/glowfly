//! Recording a rebase as one undoable action: where its branch was before it started (git's
//! orig-head while one is in progress, HEAD otherwise) and, once it has finished, where the
//! branch is now. Covers interactive rebases, the graph's history edits and rebases continued
//! from the banner, also ones started outside Glowfly.

use super::action::BranchMove;
use super::restacked::local_tips;
use super::{Action, MoveOp};
use git2::{Oid, Repository, RepositoryState};
use std::path::Path;

/// Where a rebase's branch was before it started, and the other local branches (which
/// `rebase.updateRefs` may move with it).
#[derive(Clone, Debug)]
pub struct RebaseStart {
    pub branch: String,
    pub from: Oid,
    pub interactive: bool,
    /// The other branches' tips before: every local branch, or while a rebase is in progress
    /// the ones its `update-refs` file will move.
    pub others: Vec<(String, Oid)>,
}

impl RebaseStart {
    /// "interactive rebase of main" or "rebase of main".
    pub fn describe(&self) -> String {
        let what = if self.interactive { "interactive rebase" } else { "rebase" };
        format!("{what} of {}", self.branch)
    }
}

/// The branch and commit a rebase starts from: during a rebase, its `head-name` and
/// `orig-head`; with none in progress, HEAD's branch and commit. `None` for a detached or
/// unborn HEAD, or state files that can't be read.
pub fn rebase_start(workdir: &Path) -> Option<RebaseStart> {
    let repo = Repository::open(workdir).ok()?;
    use RepositoryState::*;
    let state = repo.state();
    if matches!(state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase) {
        let dir = ["rebase-merge", "rebase-apply"].iter().map(|d| repo.path().join(d)).find(|d| d.is_dir())?;
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
        let branch = read("head-name")?.trim().strip_prefix("refs/heads/")?.to_owned();
        let from = Oid::from_str(read("orig-head")?.trim()).ok()?;
        let others = update_refs(&read("update-refs").unwrap_or_default());
        return Some(RebaseStart { branch, from, interactive: state == RebaseInteractive, others });
    }
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    let branch = head.shorthand()?.to_owned();
    let others = local_tips(&repo).into_iter().filter(|(n, _)| *n != branch).collect();
    Some(RebaseStart { branch, from: head.target()?, interactive: false, others })
}

/// The branches a rebase's `update-refs` file lists, with where each was: three lines a ref
/// (its name, where it was, where it goes).
fn update_refs(text: &str) -> Vec<(String, Oid)> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines
        .chunks(3)
        .filter_map(|c| {
            let name = c.first()?.strip_prefix("refs/heads/")?;
            let before = Oid::from_str(c.get(1)?).ok().filter(|o| !o.is_zero())?;
            Some((name.to_owned(), before))
        })
        .collect()
}

/// HEAD's branch and commit, when it is on a branch.
fn on_branch(workdir: &Path) -> Option<(String, Oid)> {
    let repo = Repository::open(workdir).ok()?;
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    Some((head.shorthand()?.to_owned(), head.target()?))
}

/// The action a finished rebase that started at `start` recorded: HEAD back on its branch, at
/// another commit. `detail` names it ("reword of 1a2b3c4"). When the rebase moved other
/// branches too (`rebase.updateRefs`), every branch it moved, undone branch by branch.
pub fn rebased(workdir: &Path, start: &RebaseStart, detail: &str) -> Option<Action> {
    let (branch, head) = on_branch(workdir)?;
    if branch != start.branch {
        return None;
    }
    let now = Repository::open(workdir).map(|r| local_tips(&r)).unwrap_or_default();
    let others: Vec<BranchMove> = start
        .others
        .iter()
        .filter_map(|(name, from)| {
            let (_, to) = now.iter().find(|(n, _)| n == name)?;
            (to != from).then(|| BranchMove { name: name.clone(), from: *from, to: *to })
        })
        .collect();
    if !others.is_empty() {
        let mut moves = Vec::new();
        if head != start.from {
            moves.push(BranchMove { name: branch, from: start.from, to: head });
        }
        moves.extend(others);
        return Some(Action::Restacked { detail: detail.to_owned(), moves, partial: true });
    }
    (head != start.from).then(|| Action::Moved {
        op: MoveOp::Rewrite,
        detail: Some(detail.to_owned()),
        branch: Some(branch),
        from: start.from,
        to: head,
    })
}

/// Whether HEAD is still on the rebase's branch at the commit it started from: nothing moved.
pub fn unmoved(workdir: &Path, start: &RebaseStart) -> bool {
    on_branch(workdir).is_some_and(|(b, h)| b == start.branch && h == start.from)
}
