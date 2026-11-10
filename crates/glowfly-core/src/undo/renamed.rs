//! Branch and remote renames: recorded when the old name is gone and the new one is there, undone
//! by renaming back (git moves a branch's config and reflog, or a remote's section, refs and
//! tracking settings, with it) and redone by renaming again. A remote edit that also changed the
//! URLs gets its earlier URLs put back too. Names are compared exactly, from a branch listing or
//! the config file, so a rename that only changes case is told apart.

use super::planning::stale;
use super::record::local_names;
use super::remote_config::{RemoteConfig, UrlChange, read, set_urls_cmds, urls};
use super::{Action, Plan, Refusal};
use crate::error::Result;
use crate::gitcli::args;
use crate::repo::BranchUse;
use git2::Repository;
use std::path::Path;

/// The rename of branch `old` to `new`, when `old` was there before (`had_old`) and `new` wasn't
/// (`new_existed`), and now `old` is gone and `new` is there.
pub(super) fn branch_after(
    repo: &Repository,
    old: String,
    new: String,
    remote: Option<String>,
    had_old: bool,
    new_existed: bool,
) -> Result<Option<Action>> {
    if !had_old || new_existed {
        return Ok(None);
    }
    let names = local_names(repo)?;
    Ok((!names.contains(&old) && names.contains(&new)).then_some(Action::RenameBranch { old, new, remote }))
}

/// Renames branch `new` back to `old` (or, redoing, `old` to `new` again), while the one it
/// renames exists, the name it takes is free and no worktree is rebasing or bisecting it.
pub(super) fn plan_branch(workdir: &Path, repo: &Repository, old: &str, new: &str, redo: bool) -> Result<Plan> {
    let (from, to) = if redo { (old, new) } else { (new, old) };
    let names = local_names(repo)?;
    if !names.contains(from) {
        return Ok(stale(format!("Branch {from} no longer exists")));
    }
    if names.contains(to) {
        return Ok(stale(format!("A branch named {to} exists again")));
    }
    for w in crate::repo::worktrees_at(workdir)? {
        if let Some(h) = w.held.as_ref().filter(|h| h.name == from) {
            let op = if h.by == BranchUse::Bisecting { "bisect" } else { "rebase" };
            let folder = w.folder_name();
            return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {op} of {from} in {folder} first."))));
        }
    }
    Ok(Plan::Run(vec![crate::branch_ops::rename_cmd(from, to)]))
}

/// The rename of remote `old` to `new`, when `old` had a section before and now only `new` has
/// one; with the URL values before and after when they changed too.
pub(super) fn remote_after(repo: &Repository, old: String, new: String, before: Option<RemoteConfig>) -> Result<Option<Action>> {
    let Some(b) = before else { return Ok(None) };
    if read(repo, &old)?.is_some() {
        return Ok(None);
    }
    let Some(a) = read(repo, &new)? else { return Ok(None) };
    let (before, after) = (urls(&b), urls(&a));
    let urls = (before != after).then_some(UrlChange { before, after });
    Ok(Some(Action::RenameRemote { old, new, urls }))
}

/// Renames remote `new` back to `old` (or `old` to `new` again), then puts the URLs the edit
/// changed back (or sets the edited ones again), while they are as the edit (or undo) left them.
pub(super) fn plan_remote(repo: &Repository, old: &str, new: &str, change: &Option<UrlChange>, redo: bool) -> Result<Plan> {
    let (from, to) = if redo { (old, new) } else { (new, old) };
    if read(repo, to)?.is_some() {
        return Ok(stale(format!("A remote named {to} exists again")));
    }
    let Some(now) = read(repo, from)? else { return Ok(stale(format!("Remote {from} no longer exists"))) };
    let mut cmds = vec![args(&["remote", "rename", "--", from, to])];
    if let Some(c) = change {
        let (expected, target) = if redo { (&c.before, &c.after) } else { (&c.after, &c.before) };
        if urls(&now) != *expected {
            return Ok(stale(format!("Remote {from} has changed since it was edited")));
        }
        cmds.extend(set_urls_cmds(to, expected, target));
    }
    Ok(Plan::Run(cmds))
}
