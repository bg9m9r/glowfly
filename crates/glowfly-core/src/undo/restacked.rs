//! Recording a restack as one undoable action: every local branch's tip before it ran, and
//! afterwards the branches whose tip changed. Undo moves each back (redo forward again): the
//! branch checked out here with `reset --keep`, the others with `update-ref` and an old-value
//! check, so a branch that moved since is refused rather than overwritten.

use super::action::BranchMove;
use super::planning::{dirty, gone, stale, tracked_clean, unsafe_};
use super::record::{branch_name, head};
use super::{Action, Plan};
use crate::error::Result;
use crate::gitcli::args;
use git2::{BranchType, Oid, Repository};
use std::path::Path;

/// Every local branch's tip before a restack.
#[derive(Clone, Debug)]
pub struct RestackBefore {
    tips: Vec<(String, Oid)>,
}

fn local_tips(repo: &Repository) -> Vec<(String, Oid)> {
    let Ok(branches) = repo.branches(Some(BranchType::Local)) else { return Vec::new() };
    branches.flatten().filter_map(|(b, _)| Some((b.name().ok().flatten()?.to_owned(), b.get().target()?))).collect()
}

/// The local branches' tips now; `None` when the repository can't be read.
pub fn restack_before(workdir: &Path) -> Option<RestackBefore> {
    let repo = Repository::open(workdir).ok()?;
    Some(RestackBefore { tips: local_tips(&repo) })
}

/// The action a finished restack recorded: the branches present before and now whose tips
/// differ. `None` when nothing moved. `detail` names it.
pub fn restacked(workdir: &Path, before: &RestackBefore, detail: &str) -> Option<Action> {
    let repo = Repository::open(workdir).ok()?;
    let now = local_tips(&repo);
    let moves: Vec<BranchMove> = before
        .tips
        .iter()
        .filter_map(|(name, from)| {
            let (_, to) = now.iter().find(|(n, _)| n == name)?;
            (to != from).then(|| BranchMove { name: name.clone(), from: *from, to: *to })
        })
        .collect();
    (!moves.is_empty()).then(|| Action::Restacked { detail: detail.to_owned(), moves })
}

/// Whether every branch is still where it was before the restack.
pub fn restack_unmoved(workdir: &Path, before: &RestackBefore) -> bool {
    Repository::open(workdir).is_ok_and(|r| local_tips(&r) == before.tips)
}

/// The commands that move every branch of a restack back (or, on redo, forward again).
pub(super) fn plan(workdir: &Path, repo: &Repository, action: &Action, redo: bool) -> Result<Plan> {
    let Action::Restacked { moves, .. } = action else { unreachable!("a restack") };
    let here = branch_name(&head(repo).0);
    let elsewhere: Vec<String> = crate::repo::worktrees_at(workdir)?.into_iter().filter(|w| !w.current).filter_map(|w| w.branch).collect();
    let reason = if redo { "Glowfly: redo restack" } else { "Glowfly: undo restack" };
    let mut cmds = Vec::new();
    let mut reset = None;
    for m in moves {
        let (expect, target) = if redo { (m.from, m.to) } else { (m.to, m.from) };
        let tip = repo.find_branch(&m.name, BranchType::Local).ok().map(|b| b.get().target());
        match tip {
            None => return Ok(stale(format!("Branch {} no longer exists", m.name))),
            Some(t) if t != Some(expect) => return Ok(stale(format!("Branch {} has moved since the {}", m.name, action.describe()))),
            Some(_) => {}
        }
        if repo.find_commit(target).is_err() {
            return Ok(gone());
        }
        if elsewhere.contains(&m.name) {
            return Ok(unsafe_(format!("{} is checked out in another worktree; switch that worktree to another branch first.", m.name)));
        }
        if here.as_deref() == Some(m.name.as_str()) {
            reset = Some(target);
        } else {
            cmds.push(args(&["update-ref", "-m", reason, &format!("refs/heads/{}", m.name), &target.to_string(), &expect.to_string()]));
        }
    }
    if let Some(target) = reset {
        if !tracked_clean(workdir)? {
            return Ok(dirty(action, redo));
        }
        cmds.push(args(&["reset", "--keep", &target.to_string()]));
    }
    Ok(Plan::Run(cmds))
}
