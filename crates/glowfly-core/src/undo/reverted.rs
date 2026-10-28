//! Reverts of part of a commit or comparison diff in the working tree, recorded for undo: the
//! working copy is saved before and after (see `saved.rs`), so undo writes back exactly what was
//! there, also when the revert merged with conflict markers, removed an added file or recreated
//! a deleted one, and redo writes the reverted copy again.

use super::saved::saving;
use super::{Action, Discarded, RestoreFailed};
use crate::error::Result;
use crate::repo::Reverted;
use std::path::Path;

/// Runs `revert` on the working-tree file `path` (`what` says which part was reverted) with its
/// contents saved first. Returns how the revert went and the action that undoes it, or `None`
/// when nothing changed or the file isn't one whose contents can be saved (the revert still
/// runs). Any other failure to save first refuses the revert; a failure to save afterwards only
/// leaves it without an undo. A revert that fails after changing the file still comes with the
/// action that puts it back (as a restore does), and its error names the blob that keeps the
/// earlier contents.
pub fn revert_saving(
    workdir: &Path,
    path: &str,
    what: Discarded,
    revert: impl FnOnce() -> Result<Reverted>,
) -> std::result::Result<(Reverted, Option<Action>), RevertFailed> {
    let action = |(before, after)| Action::Discard { path: path.to_owned(), what, before, after };
    match saving(workdir, path, false, revert) {
        Ok((reverted, pair)) => Ok((reverted, pair.map(action))),
        Err((error, pair)) => Err(RevertFailed { error, undo: pair.map(|p| Box::new(action(p))) }),
    }
}

/// A revert that failed: `undo` is set when it changed the file anyway.
pub type RevertFailed = RestoreFailed;
