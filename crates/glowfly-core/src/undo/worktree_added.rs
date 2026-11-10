//! A worktree created with a new branch (`worktree add -b`, or `--track -b` from a remote branch).
//! Undo removes the worktree without force, and only while it holds nothing but what git checked
//! out (no changed, untracked or ignored file, not locked, nothing in progress), so nothing is
//! lost; the branch is deleted too while it is still at the commit it was made at and no other
//! worktree has it. Redo makes the worktree again, with the branch when that is gone.

use super::planning::{busy_name, gone, stale, unsafe_};
use super::record::{local_branch, local_names, ref_exists};
use super::{Action, Created, Plan, Refusal};
use crate::error::Result;
use crate::gitcli::args;
use crate::paths::same_folder;
use crate::repo::{BranchUse, WorktreeInfo, worktrees_at};
use crate::worktree_ops::{RemoveForce, WorktreeBase, add_cmd, check_folder, remove_cmd};
use git2::{Repository, StatusOptions};
use std::path::Path;

/// The worktree made at `path` with new branch `branch`, when the branch didn't exist before
/// (`existed`) and both the branch and a linked worktree at `path` are there now.
pub(super) fn after(workdir: &Path, repo: &Repository, path: &Path, branch: String, existed: bool) -> Result<Option<Action>> {
    if existed || !local_names(repo)?.contains(&branch) {
        return Ok(None);
    }
    let Some((oid, upstream)) = local_branch(repo, &branch) else { return Ok(None) };
    let made = worktrees_at(workdir)?.iter().any(|w| !w.is_main() && same_folder(&w.path, path));
    let created = Created { name: branch, oid, upstream };
    Ok(made.then(|| Action::AddWorktree { path: path.to_owned(), created }))
}

/// The folder's last component, for messages.
fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Removes the worktree at `path` (and its unchanged branch), or (redoing) makes it again.
pub(super) fn plan(workdir: &Path, repo: &Repository, path: &Path, created: &Created, redo: bool) -> Result<Plan> {
    let listed = worktrees_at(workdir)?;
    if redo { plan_redo(repo, &listed, path, created) } else { plan_undo(repo, &listed, path, created) }
}

fn plan_undo(repo: &Repository, listed: &[WorktreeInfo], path: &Path, created: &Created) -> Result<Plan> {
    let folder = folder_name(path);
    let Some(at) = listed.iter().position(|w| !w.is_main() && same_folder(&w.path, path)) else {
        return Ok(stale(format!("Worktree {folder} is no longer registered at that folder")));
    };
    let wt = &listed[at];
    if wt.missing {
        return Ok(stale(format!("The folder of worktree {folder} is gone")));
    }
    if wt.locked.is_some() {
        return Ok(unsafe_(format!("Worktree {folder} is locked; unlock it first.")));
    }
    let there = Repository::open(&wt.path)?;
    let op = match &wt.held {
        Some(h) if h.by == BranchUse::Bisecting => Some("bisect"),
        Some(_) => Some("rebase"),
        None => busy_name(there.state()),
    };
    if let Some(op) = op {
        return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {op} in {folder} first."))));
    }
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).include_ignored(true).recurse_untracked_dirs(false).exclude_submodules(false);
    if !there.statuses(Some(&mut opts))?.is_empty() {
        return Ok(unsafe_(format!(
            "Worktree {folder} has changes or files made since it was created; remove it from the WORKTREES section instead."
        )));
    }
    let mut cmds = vec![remove_cmd(&wt.path, RemoveForce::None)];
    let name = &created.name;
    let unchanged = local_names(repo)?.contains(name) && local_branch(repo, name).map(|(o, _)| o) == Some(created.oid);
    let elsewhere = listed.iter().enumerate().any(|(i, w)| i != at && w.uses(name).is_some());
    if unchanged && !elsewhere {
        cmds.push(args(&["branch", "-D", name]));
    }
    Ok(Plan::Run(cmds))
}

fn plan_redo(repo: &Repository, listed: &[WorktreeInfo], path: &Path, created: &Created) -> Result<Plan> {
    let folder = folder_name(path);
    if listed.iter().any(|w| !w.is_main() && same_folder(&w.path, path)) {
        return Ok(stale(format!("Worktree {folder} exists again")));
    }
    if check_folder(path).is_err() {
        return Ok(stale(format!("The folder {folder} is in use again")));
    }
    let name = &created.name;
    if !local_names(repo)?.contains(name) {
        if repo.find_commit(created.oid).is_err() {
            return Ok(gone());
        }
        let mut cmds = vec![add_cmd(path, &WorktreeBase::Commit(created.oid), Some(name))];
        if let Some(u) = created.upstream.as_deref().filter(|u| ref_exists(repo, &format!("refs/remotes/{u}"))) {
            cmds.push(args(&["branch", "--set-upstream-to", u, name]));
        }
        return Ok(Plan::Run(cmds));
    }
    if listed.iter().any(|w| w.uses(name).is_some()) {
        return Ok(unsafe_(format!("Branch {name} is checked out in another worktree")));
    }
    Ok(Plan::Run(vec![add_cmd(path, &WorktreeBase::Branch(name.clone()), None)]))
}
