//! Merges, cherry-picks and reverts that stopped for conflicts and were finished later (Continue,
//! Skip, or a commit from the commit box): HEAD before the operation is read from the
//! repository, so the finished operation is recorded as one `Moved` step.

use super::record::branch_name;
use super::{Action, HeadPos, MoveOp, short};
use crate::error::Result;
use git2::{BranchType, Oid, Repository, RepositoryState};
use std::path::Path;

/// An operation stopped for conflicts: what it is, the commit HEAD was on before it started,
/// whether it is a sequence of picks (whose start is kept in `sequencer/head`), and the commits
/// it is merging or picking right now.
#[derive(Debug)]
pub(super) struct Paused {
    op: MoveOp,
    from: Oid,
    sequence: bool,
    picked: Vec<Oid>,
}

/// The git dir's `sequencer` folder, where git keeps a sequence of picks.
fn sequencer(repo: &Repository) -> std::path::PathBuf {
    repo.path().join("sequencer")
}

/// The operation in the first line of the sequencer's todo list that isn't a comment.
fn todo_op(repo: &Repository) -> Option<MoveOp> {
    let todo = std::fs::read_to_string(sequencer(repo).join("todo")).ok()?;
    let line = todo.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
    if line.starts_with("revert") {
        Some(MoveOp::Revert)
    } else if line.starts_with("pick") || line.starts_with("p ") {
        Some(MoveOp::CherryPick)
    } else {
        None
    }
}

/// The merge, cherry-pick or revert in progress in `workdir`, if there is one.
pub(super) fn paused(workdir: &Path) -> Result<Option<Paused>> {
    let mut repo = Repository::open(workdir)?;
    let Some(head) = repo.head().ok().and_then(|h| h.target()) else { return Ok(None) };
    let named = |repo: &Repository, name: &str| repo.refname_to_id(name).ok().into_iter().collect::<Vec<_>>();
    let (op, picked) = match repo.state() {
        RepositoryState::Merge => {
            let mut heads = Vec::new();
            repo.mergehead_foreach(|oid| {
                heads.push(*oid);
                true
            })?;
            return Ok(Some(Paused { op: MoveOp::Merge, from: head, sequence: false, picked: heads }));
        }
        RepositoryState::CherryPick | RepositoryState::CherryPickSequence => (MoveOp::CherryPick, named(&repo, "CHERRY_PICK_HEAD")),
        RepositoryState::Revert | RepositoryState::RevertSequence => (MoveOp::Revert, named(&repo, "REVERT_HEAD")),
        // A sequence whose stopped commit was committed from the commit box: git removed
        // CHERRY_PICK_HEAD (or REVERT_HEAD), so libgit2 sees nothing in progress.
        RepositoryState::Clean if sequencer(&repo).join("todo").is_file() => match todo_op(&repo) {
            Some(op) => (op, Vec::new()),
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    let start = std::fs::read_to_string(sequencer(&repo).join("head")).ok().and_then(|s| Oid::from_str(s.trim()).ok());
    Ok(Some(match start {
        Some(from) => Paused { op, from, sequence: true, picked },
        None => Paused { op, from: head, sequence: false, picked },
    }))
}

fn in_progress(repo: &Repository) -> bool {
    repo.state() != RepositoryState::Clean || sequencer(repo).is_dir()
}

/// The first local branch at `oid` (its short name), else the first remote-tracking branch
/// there (its full name), else the short commit id.
fn name_at(repo: &Repository, oid: Oid) -> String {
    let local = repo.branches(Some(BranchType::Local)).ok().and_then(|branches| {
        branches.flatten().find(|(b, _)| b.get().target() == Some(oid)).and_then(|(b, _)| b.name().ok().flatten().map(str::to_owned))
    });
    let remote = || {
        let refs = repo.references_glob("refs/remotes/*").ok()?;
        refs.flatten().find(|r| r.symbolic_target().is_none() && r.target() == Some(oid)).and_then(|r| r.name().map(str::to_owned))
    };
    local.or_else(remote).unwrap_or_else(|| short(oid))
}

/// "1 commit" or "N commits": how many commits `from..to` holds.
fn count(repo: &Repository, from: Oid, to: Oid) -> Result<String> {
    let mut walk = repo.revwalk()?;
    walk.push(to)?;
    walk.hide(from)?;
    let n = walk.count();
    Ok(if n == 1 { "1 commit".to_owned() } else { format!("{n} commits") })
}

/// The finished operation `p`, now that HEAD is at `to` (on `pos`); `None` while something is
/// still in progress or when HEAD ended where it started.
pub(super) fn finished(repo: &Repository, p: Paused, pos: &Option<HeadPos>, to: Option<Oid>) -> Result<Option<Action>> {
    let Some(to) = to.filter(|t| *t != p.from) else { return Ok(None) };
    if in_progress(repo) {
        return Ok(None);
    }
    let detail = match (p.op, p.picked.as_slice()) {
        (MoveOp::Merge, []) => None,
        (MoveOp::Merge, [one]) => Some(name_at(repo, *one)),
        (MoveOp::Merge, many) => Some(format!("{} commits", many.len())),
        (_, [one]) if !p.sequence => Some(short(*one)),
        _ => Some(count(repo, p.from, to)?),
    };
    Ok(Some(Action::Moved { op: p.op, detail, branch: branch_name(pos), from: p.from, to }))
}
