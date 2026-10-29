//! Undoing a merge, pull, cherry-pick, revert or rebase: `reset --keep` back to the commit HEAD
//! was on before (redo: forward to the one it moved to). `--keep` keeps local changes to files
//! the reset doesn't touch and refuses rather than overwrite one that it does, but Glowfly asks
//! for tracked files to be committed or stashed first so the result is the recorded one exactly.
//! Untracked files may stay, unless one (or an ignored one, which git would write over without
//! a word) is at a path the target tracks and HEAD doesn't: that is refused before local changes are
//! looked at, since stashing them wouldn't move it (`in_the_way.rs`).

use super::planning::{dirty, gone, tracked_clean};
use super::{Action, Plan};
use crate::error::Result;
use crate::gitcli::args;
use git2::{Oid, Repository};
use std::path::Path;

/// The plan for `action` once the caller has checked HEAD is where it should be: reset to
/// `target`, if that commit still exists and no tracked file has local changes.
pub(super) fn plan(workdir: &Path, repo: &Repository, action: &Action, target: Oid, redo: bool) -> Result<Plan> {
    Ok(if repo.find_commit(target).is_err() {
        gone()
    } else if let Some(refused) = super::in_the_way::check(workdir, repo, target, action, redo)? {
        // First: stashing wouldn't move it (untracked files stay for this undo).
        refused
    } else if !tracked_clean(workdir)? {
        dirty(action, redo)
    } else {
        Plan::Run(vec![args(&["reset", "--keep", &target.to_string()])])
    })
}
