//! Reverts of part of a commit or comparison diff in the working tree, recorded for undo: the
//! working copy is saved before and after (see `saved.rs`), so undo writes back exactly what was
//! there, also when the revert merged with conflict markers, removed an added file or recreated
//! a deleted one, and redo writes the reverted copy again.

use super::saved::saving;
use super::{Action, Discarded};
use crate::error::Result;
use crate::repo::Reverted;
use std::path::Path;

/// Runs `revert` on the working-tree file `path` (`what` says which part was reverted) with its
/// contents saved first. Returns how the revert went and the action that undoes it, or `None`
/// when nothing changed or the file isn't one whose contents can be saved (the revert still
/// runs). Any other failure to save first refuses the revert; a failure to save afterwards only
/// leaves it without an undo. Errors drop the action.
pub fn revert_saving(
    workdir: &Path,
    path: &str,
    what: Discarded,
    revert: impl FnOnce() -> Result<Reverted>,
) -> Result<(Reverted, Option<Action>)> {
    let (reverted, pair) = saving(workdir, path, false, revert).map_err(|(e, _)| e)?;
    Ok((reverted, pair.map(|(before, after)| Action::Discard { path: path.to_owned(), what, before, after })))
}
