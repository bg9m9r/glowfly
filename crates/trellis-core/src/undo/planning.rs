//! Planning an undo or redo: checking the repository still matches the recorded action and
//! choosing the git commands (or file write) that reverse or re-apply it.

use super::record::{head, ref_exists};
use super::{Action, HeadPos, Plan, Refusal, discard_command, reset, saved};
use crate::error::Result;
use crate::gitcli::args;
use git2::{Oid, Repository, RepositoryState};
use std::path::Path;

fn busy_name(state: RepositoryState) -> Option<&'static str> {
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

pub(super) fn stale(msg: impl Into<String>) -> Plan {
    Plan::Refuse(Refusal::Stale(format!("{}; undo history cleared", msg.into())))
}

fn gone() -> Plan {
    stale("The commit to go back to is gone (probably garbage-collected)")
}

/// The git commands that undo `action` (or redo it, after an undo), or why they can't run.
/// Checks the repository as it is now, not as a snapshot showed it.
pub fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    // Only one file's bytes change, so a merge or rebase in progress doesn't matter.
    match action {
        Action::Discard { path, before, after } => return saved::plan(workdir, path, *before, *after, redo, "discarded"),
        Action::Restore { path, before, after, .. } => return saved::plan(workdir, path, *before, *after, redo, "overwritten"),
        _ => {}
    }
    let repo = Repository::open(workdir)?;
    if let Some(op) = busy_name(repo.state()) {
        return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {op} in progress first."))));
    }
    let verb = if redo { "redoing" } else { "undoing" };
    let dirty =
        || Plan::Refuse(Refusal::Dirty(format!("Commit or stash your changes first: {verb} {} would overwrite them.", action.describe())));
    let exists = |oid: Oid| repo.find_commit(oid).is_ok();
    let (pos, head_oid) = head(&repo);
    let moved = || stale(format!("HEAD has moved since the {}", action.describe()));
    let at = |branch: &Option<String>, oid: Oid| {
        head_oid == Some(oid)
            && match (&pos, branch) {
                (Some(HeadPos::Branch(b)), Some(want)) => b == want,
                (Some(HeadPos::Detached(_)), None) => true,
                _ => false,
            }
    };
    Ok(match action {
        Action::Checkout { from, to } => {
            let (expect, target) = if redo { (from, to) } else { (to, from) };
            if pos.as_ref() != Some(expect) {
                return Ok(moved());
            }
            match target {
                HeadPos::Branch(b) if !ref_exists(&repo, &format!("refs/heads/{b}")) => stale(format!("Branch {b} no longer exists")),
                HeadPos::Branch(b) => Plan::Run(vec![args(&["checkout", b, "--"])]),
                HeadPos::Detached(o) if !exists(*o) => gone(),
                HeadPos::Detached(o) => Plan::Run(vec![args(&["checkout", "--detach", &o.to_string(), "--"])]),
            }
        }
        Action::Commit { branch, from, to, .. } => {
            let (expect, target) = if redo { (*from, *to) } else { (*to, *from) };
            if !at(branch, expect) {
                moved()
            } else if !exists(target) {
                gone()
            } else {
                Plan::Run(vec![args(&["reset", "--soft", &target.to_string()])])
            }
        }
        Action::Reset { mode, branch, from, to, stash } => {
            let (expect, target) = if redo { (*from, *to) } else { (*to, *from) };
            if !at(branch, expect) {
                moved()
            } else if !exists(target) {
                gone()
            } else {
                reset::plan(workdir, action, *mode, *stash, target, redo)?
            }
        }
        Action::DeleteBranch { name, oid, upstream } => {
            let tip = repo.find_reference(&format!("refs/heads/{name}")).ok().map(|r| r.target());
            if redo {
                match tip {
                    Some(t) if t == Some(*oid) => Plan::Run(vec![args(&["branch", "-D", name])]),
                    Some(_) => stale(format!("Branch {name} has moved since it was restored")),
                    None => stale(format!("Branch {name} no longer exists")),
                }
            } else if tip.is_some() {
                stale(format!("A branch named {name} exists again"))
            } else if !exists(*oid) {
                gone()
            } else {
                let mut cmds = vec![args(&["branch", name, &oid.to_string()])];
                if let Some(u) = upstream.as_deref().filter(|u| ref_exists(&repo, &format!("refs/remotes/{u}"))) {
                    cmds.push(args(&["branch", "--set-upstream-to", u, name]));
                }
                Plan::Run(cmds)
            }
        }
        Action::DiscardAll { stash, head } => {
            if head_oid != Some(*head) {
                return Ok(moved());
            }
            let clean = is_clean(workdir)?;
            if redo {
                return Ok(if clean { stale("No changes to discard") } else { Plan::Run(vec![discard_command()]) });
            }
            if !clean {
                return Ok(dirty());
            }
            match reset::stash_index(workdir, *stash)? {
                Some(n) => Plan::Run(vec![args(&["stash", "pop", "--index", &format!("stash@{{{n}}}")])]),
                None => stale("The discarded-changes stash was dropped"),
            }
        }
        Action::Discard { .. } | Action::Restore { .. } => unreachable!("planned above"),
    })
}
