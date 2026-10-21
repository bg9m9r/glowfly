//! Recording a rebase as one undoable action: where its branch was before it started (git's
//! orig-head while one is in progress, HEAD otherwise) and, once it has finished, where the
//! branch is now. Covers interactive rebases, the graph's history edits and rebases continued
//! from the banner, also ones started outside Glowfly.

use super::{Action, MoveOp};
use git2::{Oid, Repository, RepositoryState};
use std::path::Path;

/// Where a rebase's branch was before it started.
#[derive(Clone, Debug)]
pub struct RebaseStart {
    pub branch: String,
    pub from: Oid,
    pub interactive: bool,
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
        return Some(RebaseStart { branch, from, interactive: state == RebaseInteractive });
    }
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    Some(RebaseStart { branch: head.shorthand()?.to_owned(), from: head.target()?, interactive: false })
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
/// another commit. `detail` names it ("reword of 1a2b3c4").
pub fn rebased(workdir: &Path, start: &RebaseStart, detail: &str) -> Option<Action> {
    let (branch, head) = on_branch(workdir)?;
    (branch == start.branch && head != start.from).then(|| Action::Moved {
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
