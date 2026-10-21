//! Planning an undo or redo: checking the repository still matches the recorded action and
//! choosing the git commands (or file write) that reverse or re-apply it. Branches and tags are
//! planned in `refs.rs`, merges and the like in `moved.rs`, stashes (also dropped ones) in
//! `stash.rs`, saved conflict resolutions in `resolved.rs`.

use super::record::{head, ref_exists};
use super::{Action, HeadPos, Options, Plan, Refusal, deleted, discard_command, moved, refs, reset, resolved, restacked, saved, stash};
use crate::error::Result;
use crate::gitcli::args;
use git2::{Oid, Repository, RepositoryState};
use std::path::Path;

pub(super) fn busy_name(state: RepositoryState) -> Option<&'static str> {
    use RepositoryState::*;
    Some(match state {
        Clean => return None,
        Merge => "merge",
        Revert | RevertSequence => "revert",
        CherryPick | CherryPickSequence => "cherry-pick",
        Bisect => "bisect",
        Rebase | RebaseInteractive | RebaseMerge => "rebase",
        ApplyMailbox | ApplyMailboxOrRebase => "patch application",
    })
}

// Undo is a job the user starts, so git may read the status as for their other commands.
pub(super) fn has_staged(workdir: &Path) -> Result<bool> {
    let (_, staged, _) = crate::repo::status_at(workdir, true)?;
    Ok(!staged.is_empty())
}

pub(super) fn is_clean(workdir: &Path) -> Result<bool> {
    let (unstaged, staged, _) = crate::repo::status_at(workdir, true)?;
    Ok(unstaged.is_empty() && staged.is_empty())
}

/// No staged or unstaged changes to tracked files (untracked files don't count).
pub(super) fn tracked_clean(workdir: &Path) -> Result<bool> {
    let (unstaged, staged, _) = crate::repo::status_at(workdir, true)?;
    Ok(staged.is_empty() && unstaged.iter().all(|f| f.status == '?'))
}

pub(super) fn stale(msg: impl Into<String>) -> Plan {
    Plan::Refuse(Refusal::Stale(format!("{}; undo history cleared", msg.into())))
}

pub(super) fn gone() -> Plan {
    stale("The commit to go back to is gone (probably garbage-collected)")
}

pub(super) fn unsafe_(msg: impl Into<String>) -> Plan {
    Plan::Refuse(Refusal::Unsafe(msg.into()))
}

/// The refusal for local changes that undoing (or redoing) `action` would overwrite.
pub(super) fn dirty(action: &Action, redo: bool) -> Plan {
    let verb = if redo { "redoing" } else { "undoing" };
    Plan::Refuse(Refusal::Dirty(format!("Commit or stash your changes first: {verb} {} would overwrite them.", action.describe())))
}

/// Whether HEAD (at `pos`, commit `head_oid`) is commit `oid` on `branch` (`None`: detached).
pub(super) fn at(pos: &Option<HeadPos>, head_oid: Option<Oid>, branch: &Option<String>, oid: Oid) -> bool {
    head_oid == Some(oid)
        && match (pos, branch) {
            (Some(HeadPos::Branch(b)), Some(want)) => b == want,
            (Some(HeadPos::Detached(_)), None) => true,
            _ => false,
        }
}

/// The git commands that undo `action` (or redo it, after an undo), or why they can't run.
/// Checks the repository as it is now, not as a snapshot showed it.
pub fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    plan_with(workdir, action, redo, Options::default())
}

/// `plan`, with the choices the user made for this undo.
pub fn plan_with(workdir: &Path, action: &Action, redo: bool, opts: Options) -> Result<Plan> {
    // Only one file's bytes change, so a merge or rebase in progress doesn't matter.
    match action {
        Action::Discard { path, what, before, after } => return saved::plan(workdir, path, *before, *after, redo, what.kept_as()),
        Action::Restore { path, before, after, .. } => return saved::plan(workdir, path, *before, *after, redo, "overwritten"),
        // A staged entry is checked against an operation in progress there.
        Action::DeleteFile { .. } => return deleted::plan(workdir, action, redo),
        // A stash store or drop touches no HEAD, index or file, so an operation in progress
        // doesn't matter either.
        Action::StashDrop { .. } => {
            let repo = Repository::open(workdir)?;
            return stash::plan_drop(workdir, &repo, action, redo);
        }
        // A saved conflict resolution is expected to have an operation in progress.
        Action::Resolve { .. } => return resolved::plan(workdir, action, redo),
        _ => {}
    }
    let repo = Repository::open(workdir)?;
    if let Some(op) = busy_name(repo.state()) {
        return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {op} in progress first."))));
    }
    let exists = |oid: Oid| repo.find_commit(oid).is_ok();
    let (pos, head_oid) = head(&repo);
    let moved = || stale(format!("HEAD has moved since the {}", action.describe()));
    Ok(match action {
        Action::Checkout { from, to, created } => {
            let (expect, target) = if redo { (from, to) } else { (to, from) };
            if pos.as_ref() != Some(expect) {
                return Ok(moved());
            }
            let mut cmds = match target {
                HeadPos::Branch(b) if !ref_exists(&repo, &format!("refs/heads/{b}")) => {
                    return Ok(match created {
                        Some(c) if redo && c.name == *b => refs::recreate(&repo, c),
                        _ => stale(format!("Branch {b} no longer exists")),
                    });
                }
                HeadPos::Branch(b) => vec![args(&["checkout", b, "--"])],
                HeadPos::Detached(o) if !exists(*o) => return Ok(gone()),
                HeadPos::Detached(o) => vec![args(&["checkout", "--detach", &o.to_string(), "--"])],
            };
            if let (false, true, Some(c)) = (redo, opts.delete_created, created) {
                match refs::delete_created(&repo, c) {
                    Ok(cmd) => cmds.push(cmd),
                    Err(refusal) => return Ok(refusal),
                }
            }
            Plan::Run(cmds)
        }
        Action::Commit { branch, from, to, .. } => {
            let (expect, target) = if redo { (*from, *to) } else { (*to, *from) };
            if !at(&pos, head_oid, branch, expect) {
                moved()
            } else if !exists(target) {
                gone()
            } else {
                Plan::Run(vec![args(&["reset", "--soft", &target.to_string()])])
            }
        }
        Action::Reset { mode, branch, from, to, stash } => {
            let (expect, target) = if redo { (*from, *to) } else { (*to, *from) };
            if !at(&pos, head_oid, branch, expect) {
                moved()
            } else if !exists(target) {
                gone()
            } else {
                reset::plan(workdir, action, *mode, *stash, target, redo)?
            }
        }
        Action::DeleteBranch { name, oid, .. } if redo => {
            refs::remove_branch(&repo, &pos, name, *oid, format!("Branch {name} has moved since it was restored"))
        }
        Action::DeleteBranch { name, oid, upstream } => refs::restore_branch(&repo, name, *oid, upstream.as_deref()),
        Action::CreateBranch { name, oid, upstream } if redo => refs::restore_branch(&repo, name, *oid, upstream.as_deref()),
        Action::CreateBranch { name, oid, .. } => {
            refs::remove_branch(&repo, &pos, name, *oid, format!("Branch {name} has moved since it was created"))
        }
        Action::CreateTag { name, oid } => refs::tag(&repo, name, *oid, redo),
        Action::SetBranch { name, from, to } => refs::set_branch(&repo, &pos, name, *from, *to, redo),
        Action::Restacked { .. } => restacked::plan(workdir, &repo, action, redo)?,
        Action::Moved { branch, from, to, .. } => {
            let (expect, target) = if redo { (*from, *to) } else { (*to, *from) };
            if !at(&pos, head_oid, branch, expect) {
                let who = branch.as_deref().unwrap_or("HEAD");
                stale(format!("{who} has moved since the {}", action.describe()))
            } else {
                moved::plan(workdir, &repo, action, target, redo)?
            }
        }
        Action::StashPush { .. } => stash::plan_push(workdir, &repo, action, head_oid, redo)?,
        Action::StashApply { .. } => stash::plan_apply(workdir, &repo, action, head_oid, redo)?,
        Action::DiscardAll { stash, head } => {
            if head_oid != Some(*head) {
                return Ok(moved());
            }
            let clean = is_clean(workdir)?;
            if redo {
                return Ok(if clean { stale("No changes to discard") } else { Plan::Run(vec![discard_command()]) });
            }
            if !clean {
                return Ok(dirty(action, redo));
            }
            match reset::stash_index(workdir, *stash)? {
                Some(n) => Plan::Run(vec![args(&["stash", "pop", "--index", &format!("stash@{{{n}}}")])]),
                None => stale("The discarded-changes stash was dropped"),
            }
        }
        Action::Discard { .. } | Action::Restore { .. } | Action::StashDrop { .. } | Action::Resolve { .. } => {
            unreachable!("planned above")
        }
        Action::DeleteFile { .. } => unreachable!("planned above"),
    })
}
