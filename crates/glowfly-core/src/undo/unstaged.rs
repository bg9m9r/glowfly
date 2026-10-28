//! Undo for Unstage deleted file: a file staged as new (or as a staged rename's new path) that
//! is already gone from the working tree is taken out of the index with its staged entry kept
//! first; its blob is already in the object database. Undo puts the entry back (and takes a
//! rename's source out of the index again); redo takes it out again. Nothing on disk changes.

use super::planning::{busy_name, stale};
use super::{Action, Plan, Refusal};
use crate::error::Result;
use crate::repo;
use git2::Repository;
use std::path::Path;

/// Takes `path` out of the index (see [`repo::unstage_gone_new_file`]), keeping its staged
/// entry first. Returns the action that undoes it.
pub fn unstage_gone_saving(workdir: &Path, path: &str) -> Result<Action> {
    let staged = repo::staged_new(workdir, path)?;
    repo::unstage_gone_new_file(workdir, path)?;
    Ok(Action::UnstageGone { path: path.to_owned(), staged })
}

/// Undo restages when the index is as the unstage left it and the blob is still there; redo
/// unstages again when the index still holds what undo put back.
pub(super) fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    let Action::UnstageGone { path, staged } = action else { unreachable!("an unstage") };
    let git = Repository::open(workdir)?;
    if let Some(op) = busy_name(git.state()) {
        return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {op} in progress first."))));
    }
    if redo {
        if !repo::staged_new_matches(workdir, path, staged)? {
            return Ok(stale(format!("{path}'s staged version has changed since it was staged again")));
        }
        return Ok(Plan::UnstageAgain { path: path.clone() });
    }
    if git.find_blob(staged.blob).is_err() {
        return Ok(stale(format!("The staged version of {path} is gone")));
    }
    if !repo::index_clear_for(workdir, path, staged)? {
        return Ok(stale(format!("{path} has been staged again since")));
    }
    Ok(Plan::Restage { path: path.clone(), staged: staged.clone() })
}

/// Puts `staged` back in the index at `path`.
pub fn restage(workdir: &Path, path: &str, staged: &repo::StagedNew) -> Result<()> {
    repo::restage_new(workdir, path, staged)
}

/// Takes `path` out of the index again.
pub fn unstage_again(workdir: &Path, path: &str) -> Result<()> {
    repo::unstage_gone_new_file(workdir, path)
}
