//! Resets and their undo. A hard reset first stashes uncommitted changes to tracked files, so
//! undoing it brings them back along with the old HEAD.

use super::{Action, Plan, Refusal, ResetMode, has_staged, is_clean, stale};
use crate::error::Result;
use crate::gitcli::args;
use git2::{Oid, Repository};
use std::path::Path;

/// The message of the stash a hard reset moves uncommitted changes into.
pub const HARD_RESET_STASH_MESSAGE: &str = "Trellis: changes before hard reset";

/// The commands Trellis runs for a hard reset to `rev`: stash the changes to tracked files
/// (git saves nothing and goes on when there are none), then reset.
pub fn hard_reset_commands(rev: &str) -> Vec<Vec<String>> {
    vec![args(&["stash", "push", "-m", HARD_RESET_STASH_MESSAGE]), args(&["reset", "--hard", rev])]
}

/// The position of stash `oid` in the stash list, if it is still there.
pub(super) fn stash_index(workdir: &Path, oid: Oid) -> Result<Option<usize>> {
    let mut repo = Repository::open(workdir)?;
    let mut index = None;
    repo.stash_foreach(|i, _, id| {
        if *id == oid {
            index = Some(i);
        }
        index.is_none()
    })?;
    Ok(index)
}

/// The stash a hard reset from `from` just made, if the top of the stash list is one.
pub(super) fn new_stash(repo: &Repository, from: Oid) -> Option<Oid> {
    let top = super::stash_top(repo)?;
    let c = repo.find_commit(top).ok()?;
    let ours = c.message().is_some_and(|m| m.contains(HARD_RESET_STASH_MESSAGE)) && c.parent_id(0).ok() == Some(from);
    ours.then_some(top)
}

/// Undoes (or redoes) a reset whose HEAD and target were checked by the caller: `target` is the
/// commit to reset to and `stash` holds what a hard reset stashed.
pub(super) fn plan(workdir: &Path, action: &Action, mode: ResetMode, stash: Option<Oid>, target: Oid, redo: bool) -> Result<Plan> {
    let verb = if redo { "redoing" } else { "undoing" };
    let target = target.to_string();
    Ok(match mode {
        // Stashes again whatever is uncommitted now, as the original reset did.
        ResetMode::Hard if redo => Plan::Run(hard_reset_commands(&target)),
        ResetMode::Hard => {
            if !is_clean(workdir)? {
                return Ok(Plan::Refuse(Refusal::Dirty(format!(
                    "Commit or stash your changes first: {verb} {} would overwrite them.",
                    action.describe()
                ))));
            }
            let mut cmds = vec![args(&["reset", "--hard", &target])];
            if let Some(s) = stash {
                match stash_index(workdir, s)? {
                    Some(n) => cmds.push(args(&["stash", "pop", "--index", &format!("stash@{{{n}}}")])),
                    None => return Ok(stale("The stash holding the changes from before the hard reset was dropped")),
                }
            }
            Plan::Run(cmds)
        }
        // A mixed reset rewrites the index, so a staged version that differs from the working
        // copy would be lost.
        ResetMode::Mixed if has_staged(workdir)? => Plan::Refuse(Refusal::Dirty(format!(
            "Commit or unstage your staged changes first: {verb} {} would replace them.",
            action.describe()
        ))),
        _ => Plan::Run(vec![args(&["reset", mode.flag(), &target])]),
    })
}
