//! Local branches and worktrees together: which branches another worktree has checked out, or is
//! rebasing or bisecting with HEAD detached (git refuses to check them out, delete or
//! fast-forward them here, and to rename one being rebased or bisected), the words for that, and
//! the bases a worktree is created from for a remote branch or a pull request, with the rules
//! for the new local branch's name. Shared by the views and `update`, so a disabled item and a
//! refused request say the same thing. Pure: a branch is matched to a worktree by its exact name
//! (`WorktreeInfo::uses`), and the current worktree is the one the listing marked
//! (`WorktreeInfo::current`, by identity).

use super::branches::{pr_source_missing, pr_source_refused};
use glowfly_core::branch_ops::check_branch_name;
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_core::repo::{BranchInfo, BranchUse, Snapshot, WorktreeInfo};
use glowfly_core::worktree_ops::WorktreeBase;

/// The worktree other than the current one that has local branch `branch` checked out, or is
/// rebasing or bisecting it. A missing worktree counts: git refuses the branch until its record
/// is pruned.
pub fn elsewhere<'a>(snap: &'a Snapshot, branch: &str) -> Option<&'a WorktreeInfo> {
    snap.worktrees.iter().find(|w| !w.current && w.uses(branch).is_some())
}

/// `elsewhere`, with how that worktree has the branch.
fn used_elsewhere<'a>(snap: &'a Snapshot, branch: &str) -> Option<(&'a WorktreeInfo, BranchUse)> {
    elsewhere(snap, branch).and_then(|w| Some((w, w.uses(branch)?)))
}

/// Why `branch` is refused because of `wt`: its folder is missing, or it is rebasing or
/// bisecting the branch there. None when it only has it checked out.
fn held_text(t: Lang, branch: &str, wt: &WorktreeInfo, by: BranchUse) -> Option<String> {
    let args = [("branch", &branch as &dyn std::fmt::Display), ("path", &shown(&wt.path))];
    Some(match by {
        _ if wt.missing => t.trf("worktree_branch.blocked.missing", &args),
        BranchUse::Rebasing => t.trf("worktree_branch.blocked.rebasing", &args),
        BranchUse::Bisecting => t.trf("worktree_branch.blocked.bisecting", &args),
        BranchUse::CheckedOut => return None,
    })
}

/// Why local branch `branch` can't be checked out (or fast-forwarded) here: another worktree has
/// it.
pub fn checkout_blocked(t: Lang, snap: &Snapshot, branch: &str) -> Option<String> {
    let (wt, by) = used_elsewhere(snap, branch)?;
    held_text(t, branch, wt, by)
        .or_else(|| Some(t.trf("worktree_branch.blocked.checkout", &[("branch", &branch), ("path", &shown(&wt.path))])))
}

/// Why local branch `branch` can't be deleted: another worktree has it.
pub fn delete_blocked(t: Lang, snap: &Snapshot, branch: &str) -> Option<String> {
    let (wt, by) = used_elsewhere(snap, branch)?;
    held_text(t, branch, wt, by)
        .or_else(|| Some(t.trf("worktree_branch.blocked.delete", &[("branch", &branch), ("path", &shown(&wt.path))])))
}

/// Why local branch `branch` can't be renamed or force pushed: another worktree is rebasing or
/// bisecting it (git renames a branch merely checked out there, and moves that HEAD with it).
pub fn held_blocked(t: Lang, snap: &Snapshot, branch: &str) -> Option<String> {
    let (wt, by) = used_elsewhere(snap, branch)?;
    (by != BranchUse::CheckedOut).then(|| held_text(t, branch, wt, by)).flatten()
}

/// Why `b` can't be fast-forwarded to its upstream: when it isn't checked out here, another
/// worktree has it (git won't move it under that worktree); else `branches::fast_forward_blocked`.
pub fn fast_forward_blocked(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Option<String> {
    let elsewhere = (!b.is_head).then(|| checkout_blocked(t, snap, &b.name)).flatten();
    elsewhere.or_else(|| super::branches::fast_forward_blocked(b, snap.state))
}

/// The LOCAL row's tooltip line for local branch `branch`, which `wt` has.
pub fn local_tip_line(t: Lang, wt: &WorktreeInfo, branch: &str) -> String {
    let path = shown(&wt.path);
    let line = match wt.uses(branch) {
        Some(BranchUse::Rebasing) => t.trf("worktree_branch.tip.rebasing", &[("path", &path)]),
        Some(BranchUse::Bisecting) => t.trf("worktree_branch.tip.bisecting", &[("path", &path)]),
        _ => t.trf("worktree_branch.tip.checked_out", &[("path", &path)]),
    };
    if wt.missing { t.trf("worktree_branch.tip.missing", &[("line", &line)]) } else { line }
}

/// What a worktree for a pull request's source branch `source` is created from: the local
/// branch of that name, else `<remote>/<source>`. Refused like a checkout of it
/// (`branches::pr_checkout`): a name git would read as an option, or one this repository
/// doesn't have.
pub fn pr_base(snap: &Snapshot, remote: &str, source: &str) -> Result<WorktreeBase, String> {
    if let Some(why) = pr_source_refused(source) {
        return Err(why);
    }
    if snap.locals.iter().any(|b| b.name == source) {
        return Ok(WorktreeBase::Branch(source.to_owned()));
    }
    let on_remote = snap.remotes.iter().any(|(r, bs)| r == remote && bs.iter().any(|(n, _)| n == source));
    if on_remote {
        return Ok(WorktreeBase::Remote { remote: remote.to_owned(), branch: source.to_owned() });
    }
    Err(pr_source_missing(source, remote))
}

/// What makes `<remote>/<branch>` and the new local branch's name `new` unusable for a worktree:
/// the remote branch is gone, or the name is empty, invalid or taken (in its own words when the
/// branch of that name already tracks this remote branch).
pub fn remote_problem(snap: &Snapshot, remote: &str, branch: &str, new: &str) -> Option<String> {
    let full = format!("{remote}/{branch}");
    let exists = snap.remotes.iter().any(|(r, bs)| r == remote && bs.iter().any(|(n, _)| n == branch));
    if !exists {
        return Some(format!("{full} no longer exists. Fetch, then try again."));
    }
    let new = new.trim();
    if new.is_empty() {
        return Some("Enter a name for the new local branch.".to_owned());
    }
    let tracks = |b: &glowfly_core::repo::BranchInfo| b.tracking.as_ref().is_some_and(|t| t.remote == remote && t.merge == branch);
    if let Some(b) = snap.locals.iter().find(|b| b.name == new && tracks(b)) {
        return Some(format!("{} already tracks {full}. Create the worktree from {} instead, or enter another name.", b.name, b.name));
    }
    let existing: Vec<&str> = snap.locals.iter().map(|l| l.name.as_str()).collect();
    check_branch_name(new, &existing, None).err().map(|p| p.message(new))
}
