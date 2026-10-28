//! Undo for Delete…, Unstage and delete… and Discard of an untracked file: the working copy's
//! exact bytes are kept as a blob before the file goes (as a discard keeps them), and for a
//! staged new file its index entry too, whose blob is already in the object database. A delete
//! that fails after the file went (the trash reported an error, or the index couldn't be
//! changed) keeps its undo too. Undo takes the file back out of the Recycle Bin / Trash when it
//! went there and the trash still holds those bytes, and otherwise writes the kept copy back
//! (never over anything); a staged one is staged again. Redo deletes it again the way it went
//! the first time.

use super::planning::{busy_name, stale, unsafe_};
use super::saved::RestoreFailed;
use super::{Action, Plan, Refusal, Saved, saved};
use crate::error::{Error, Result};
use crate::repo::{self, Deleted, Removal, StagedNew, Trasher, Untrashed, Untrasher};
use git2::Repository;
use std::path::Path;

/// How a deleted file went, so a redo can delete it the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteVia {
    /// To the system trash (the Recycle Bin, the Trash).
    Trash,
    /// Copied into the Trash in the home folder (Linux), the drive's own trash being unusable.
    HomeTrash,
    /// Removed for good.
    Permanent,
}

/// The trashes a delete can use: the system's, and the home Trash for a file whose drive has
/// none Glowfly can use, and how Undo takes a file back out of either. Production code passes
/// `repo::os_trash`, `repo::home_trash` and `repo::os_untrash`; tests pass stand-ins.
#[derive(Clone, Copy, Debug)]
pub struct Trashers {
    pub system: Trasher,
    pub home: Trasher,
    pub restore: Untrasher,
}

impl Trashers {
    /// How a delete via `via` removes the working copy.
    pub fn removal(self, via: DeleteVia) -> Removal {
        match via {
            DeleteVia::Trash => Removal::Trash(self.system),
            DeleteVia::HomeTrash => Removal::Trash(self.home),
            DeleteVia::Permanent => Removal::Permanent,
        }
    }
}

/// Files larger than this are deleted without a copy kept for undo.
pub const MAX_KEPT_BYTES: u64 = 64 * 1024 * 1024;

/// A delete that ran: how the working copy went, the action that undoes it (none for a
/// symbolic link, or a file too large to keep a copy of), and whether it was too large.
#[derive(Debug)]
pub struct DeletedSaving {
    pub how: Deleted,
    pub undo: Option<Action>,
    pub too_large: bool,
}

/// A delete that failed: `undo` is set when the file went anyway, so it can still be put back.
pub type DeleteFailed = RestoreFailed;

type Deleting = std::result::Result<DeletedSaving, DeleteFailed>;

/// Deletes the working-tree file `path` (see [`repo::delete_file`]) via `via`, keeping its
/// bytes first so the delete can be undone. A failure to keep them refuses the delete.
pub fn delete_saving(workdir: &Path, path: &str, via: DeleteVia, t: Trashers) -> Deleting {
    delete_saving_with(workdir, path, via, t, MAX_KEPT_BYTES)
}

/// `delete_saving` with the size limit given, for tests.
pub(super) fn delete_saving_with(workdir: &Path, path: &str, via: DeleteVia, t: Trashers, limit: u64) -> Deleting {
    saving(workdir, path, via, None, limit, || repo::delete_file(workdir, path, t.removal(via)))
}

/// Discards the untracked file `path` (see [`repo::discard_untracked`]) via `via`, keeping its
/// bytes first so the discard can be undone like a delete.
pub fn discard_untracked_saving(workdir: &Path, path: &str, via: DeleteVia, t: Trashers) -> Deleting {
    saving(workdir, path, via, None, MAX_KEPT_BYTES, || repo::discard_untracked(workdir, path, t.removal(via)))
}

/// Unstage and delete (see [`repo::delete_staged_new_file`]) via `via`, keeping the working
/// copy's bytes and the staged entry first.
pub fn delete_staged_saving(workdir: &Path, path: &str, via: DeleteVia, t: Trashers) -> Deleting {
    let staged = repo::staged_new(workdir, path).map_err(refused)?;
    saving(workdir, path, via, Some(staged), MAX_KEPT_BYTES, || repo::delete_staged_new_file(workdir, path, t.removal(via)))
}

fn refused(error: Error) -> DeleteFailed {
    DeleteFailed { error, undo: None }
}

/// Keeps `path`'s bytes, then runs `delete`. When that fails after the file went, the index is
/// as it was (it changes last, in one write), so the undo entry puts back only the file.
fn saving(
    workdir: &Path,
    path: &str,
    via: DeleteVia,
    staged: Option<StagedNew>,
    limit: u64,
    delete: impl FnOnce() -> Result<Deleted>,
) -> Deleting {
    let (before, too_large) = keep(workdir, path, limit).map_err(refused)?;
    match delete() {
        Ok(how) => Ok(finish(path, how, before, via, staged, too_large)),
        Err(error) => {
            let gone = !matches!(error, Error::TrashRefused(_))
                && repo::contained(path).is_ok_and(|rel| workdir.join(rel).symlink_metadata().is_err());
            let undo =
                before.filter(|_| gone).map(|before| Box::new(Action::DeleteFile { path: path.to_owned(), before, via, staged: None }));
            Err(DeleteFailed { error, undo })
        }
    }
}

/// The working copy's bytes saved as a blob, when it is a regular file of at most `limit`
/// bytes, and whether it was larger. Links and folders aren't kept.
fn keep(workdir: &Path, path: &str, limit: u64) -> Result<(Option<Saved>, bool)> {
    let full = workdir.join(repo::contained(path)?);
    match std::fs::symlink_metadata(full) {
        Ok(m) if m.is_file() && m.len() > limit => Ok((None, true)),
        Ok(m) if m.is_file() => match saved::save(workdir, path) {
            Ok(s) => Ok((s, false)),
            Err(Error::Unsupported(_)) => Ok((None, false)),
            Err(e) => Err(e),
        },
        _ => Ok((None, false)),
    }
}

fn finish(path: &str, how: Deleted, before: Option<Saved>, via: DeleteVia, staged: Option<StagedNew>, too_large: bool) -> DeletedSaving {
    let undo =
        before.filter(|_| how != Deleted::LinkRemoved).map(|before| Action::DeleteFile { path: path.to_owned(), before, via, staged });
    DeletedSaving { how, undo, too_large }
}

/// Undo puts the file back when nothing is at its path (and, for a staged one, the index is
/// as Unstage and delete left it); redo deletes it again when it still holds what was put back.
pub(super) fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    let Action::DeleteFile { path, before, via, staged } = action else { unreachable!("a delete") };
    let git = Repository::open(workdir)?;
    // A plain delete's undo changes only one file, like a discard's; a staged one changes the index.
    if staged.is_some()
        && let Some(op) = busy_name(git.state())
    {
        return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {op} in progress first."))));
    }
    if redo {
        let now = match saved::current(workdir, path) {
            Ok(n) => n,
            Err(Error::Unsupported(m)) => return Ok(stale(m)),
            Err(e) => return Err(e),
        };
        if now.map(|s| s.blob) != Some(before.blob) {
            return Ok(stale(format!("{path} has changed since it was put back")));
        }
        if let Some(s) = staged
            && !repo::staged_new_matches(workdir, path, s)?
        {
            return Ok(stale(format!("{path}'s staged version has changed since it was put back")));
        }
        return Ok(Plan::DeleteAgain { path: path.clone(), via: *via, staged: staged.is_some() });
    }
    // Anything there now, a case twin on a case-insensitive volume too, is someone's work.
    if workdir.join(repo::contained(path)?).symlink_metadata().is_ok() {
        let b = before.blob;
        return Ok(unsafe_(format!(
            "{path} is in your working tree again; move it away to undo the delete. The deleted version is kept as blob {b} (git cat-file -p {b})."
        )));
    }
    if git.find_blob(before.blob).is_err() {
        return Ok(stale(format!("The deleted version of {path} is gone")));
    }
    if let Some(s) = staged
        && (git.find_blob(s.blob).is_err() || !repo::index_clear_for(workdir, path, s)?)
    {
        return Ok(stale(format!("{path} has been staged again since")));
    }
    Ok(Plan::PutBack { path: path.clone(), content: *before, staged: staged.clone(), via: *via })
}

/// How Undo put a deleted file back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PutBackHow {
    /// Taken back out of the Recycle Bin / Trash.
    FromTrash,
    /// The kept copy was written; `trash` is what the trash said, for a file that went there.
    Written { trash: Option<Untrashed> },
}

impl PutBackHow {
    /// The output line for putting back `path` (`staged`: staged again too), deleted via `via`.
    pub fn wrote(&self, path: &str, staged: bool, via: DeleteVia) -> String {
        let again = if staged { " and staged it again" } else { "" };
        let bin = match via {
            DeleteVia::HomeTrash => "Trash in your home folder",
            _ => repo::TRASH_NAME,
        };
        match self {
            PutBackHow::FromTrash => format!("Took {path} back out of the {bin}{again}"),
            PutBackHow::Written { trash: None | Some(Untrashed::Restored) } => format!("Put back {path}{again}"),
            PutBackHow::Written { trash: Some(Untrashed::NotFound) } => {
                format!("Put back {path}{again} from Glowfly's copy; the {bin} no longer holds it as it was deleted")
            }
            PutBackHow::Written { trash: Some(Untrashed::Unsupported) } => {
                format!("Put back {path}{again} from Glowfly's copy (its copy in the {bin} stays there)")
            }
            PutBackHow::Written { trash: Some(Untrashed::Failed(why)) } => {
                format!("Put back {path}{again} from Glowfly's copy (its copy in the {bin} stays there: {why})")
            }
        }
    }
}

/// Puts the deleted file `path` back (it must be absent), with `staged` back in the index: out
/// of the trash when it went there via `via` and the trash still holds exactly `content`'s
/// bytes, otherwise by writing `content` with the executable bit unless the repository ignores
/// it (`core.fileMode=false`).
pub fn put_back(workdir: &Path, path: &str, content: Saved, staged: Option<&StagedNew>, via: DeleteVia, t: Trashers) -> Result<PutBackHow> {
    let trash = match via {
        DeleteVia::Permanent => None,
        DeleteVia::Trash | DeleteVia::HomeTrash => match repo::untrash_file(workdir, path, content.blob, staged, t.restore)? {
            Untrashed::Restored => return Ok(PutBackHow::FromTrash),
            other => Some(other),
        },
    };
    let (bytes, file_mode) = {
        let git = Repository::open(workdir)?;
        let bytes = git.find_blob(content.blob)?.content().to_vec();
        (bytes, git.config().and_then(|c| c.get_bool("core.fileMode")).unwrap_or(true))
    };
    repo::undelete_file(workdir, path, &bytes, file_mode.then_some(content.executable), staged)?;
    Ok(PutBackHow::Written { trash })
}

/// Deletes `path` again via `via`, unstaging it too when it was `staged` as new.
pub fn delete_again(workdir: &Path, path: &str, via: DeleteVia, staged: bool, t: Trashers) -> Result<Deleted> {
    if staged { repo::delete_staged_new_file(workdir, path, t.removal(via)) } else { repo::delete_file(workdir, path, t.removal(via)) }
}
