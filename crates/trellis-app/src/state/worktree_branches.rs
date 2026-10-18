//! Local branches and worktrees together: which branches are checked out in another worktree
//! (git refuses to check them out or delete them here), the words for that, and the bases a
//! worktree is created from for a remote branch or a pull request, with the rules for the new
//! local branch's name. Shared by the views and `update`, so a disabled item and a refused
//! request say the same thing. Pure: a branch is matched to a worktree by its exact name, and
//! the current worktree is the one the listing marked (`WorktreeInfo::current`, by identity).

use super::branches::{pr_source_missing, pr_source_refused};
use trellis_core::branch_ops::check_branch_name;
use trellis_core::native_path::shown;
use trellis_core::repo::{Snapshot, WorktreeInfo};
use trellis_core::worktree_ops::WorktreeBase;

/// The worktree other than the current one that has local branch `branch` checked out. A
/// missing worktree counts: git refuses the branch until its record is pruned.
pub fn elsewhere<'a>(snap: &'a Snapshot, branch: &str) -> Option<&'a WorktreeInfo> {
    snap.worktrees.iter().find(|w| !w.current && w.branch.as_deref() == Some(branch))
}

/// "<b> is checked out in the worktree at <path>, whose folder is missing. Prune it first."
fn missing_text(branch: &str, wt: &WorktreeInfo) -> String {
    format!("{branch} is checked out in the worktree at {}, whose folder is missing. Prune it first.", shown(&wt.path))
}

/// Why local branch `branch` can't be checked out here: another worktree has it.
pub fn checkout_blocked(snap: &Snapshot, branch: &str) -> Option<String> {
    let wt = elsewhere(snap, branch)?;
    Some(if wt.missing {
        missing_text(branch, wt)
    } else {
        format!("{branch} is checked out in the worktree at {}. Open that worktree instead.", shown(&wt.path))
    })
}

/// Why local branch `branch` can't be deleted: another worktree has it.
pub fn delete_blocked(snap: &Snapshot, branch: &str) -> Option<String> {
    let wt = elsewhere(snap, branch)?;
    Some(if wt.missing {
        missing_text(branch, wt)
    } else {
        format!(
            "{branch} is checked out in the worktree at {}. Remove that worktree or check out another branch there first.",
            shown(&wt.path)
        )
    })
}

/// The LOCAL row's tooltip line for a branch checked out in `wt`.
pub fn local_tip_line(wt: &WorktreeInfo) -> String {
    let mut line = format!("Checked out in the worktree at {}", shown(&wt.path));
    if wt.missing {
        line.push_str(" (folder missing; prune it first)");
    }
    line
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
    let tracks = |b: &trellis_core::repo::BranchInfo| b.tracking.as_ref().is_some_and(|t| t.remote == remote && t.merge == branch);
    if let Some(b) = snap.locals.iter().find(|b| b.name == new && tracks(b)) {
        return Some(format!("{} already tracks {full}. Create the worktree from {} instead, or enter another name.", b.name, b.name));
    }
    let existing: Vec<&str> = snap.locals.iter().map(|l| l.name.as_str()).collect();
    check_branch_name(new, &existing, None).err().map(|p| p.message(new))
}
