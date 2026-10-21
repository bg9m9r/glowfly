//! Undo for Delete… and Unstage and delete…: the working copy's exact bytes are kept as a blob
//! before the file goes (as a discard keeps them), and for a staged new file its index entry
//! too, whose blob is already in the object database. Undo writes the file back (never over
//! anything) and stages it again; any copy in the Recycle Bin / Trash stays there. Redo deletes
//! it again the way it went the first time.

use super::planning::{busy_name, stale, unsafe_};
use super::{Action, Plan, Refusal, Saved, saved};
use crate::error::{Error, Result};
use crate::repo::{self, Deleted, Removal, StagedNew, Trasher};
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
/// none Glowfly can use. Production code passes `repo::os_trash` and `repo::home_trash`; tests
/// pass stand-ins.
#[derive(Clone, Copy, Debug)]
pub struct Trashers {
    pub system: Trasher,
    pub home: Trasher,
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

/// Deletes the working-tree file `path` (see [`repo::delete_file`]) via `via`, keeping its
/// bytes first so the delete can be undone. A failure to keep them refuses the delete.
pub fn delete_saving(workdir: &Path, path: &str, via: DeleteVia, t: Trashers) -> Result<DeletedSaving> {
    delete_saving_with(workdir, path, via, t, MAX_KEPT_BYTES)
}

/// `delete_saving` with the size limit given, for tests.
pub(super) fn delete_saving_with(workdir: &Path, path: &str, via: DeleteVia, t: Trashers, limit: u64) -> Result<DeletedSaving> {
    let (before, too_large) = keep(workdir, path, limit)?;
    let how = repo::delete_file(workdir, path, t.removal(via))?;
    Ok(finish(path, how, before, via, None, too_large))
}

/// Unstage and delete (see [`repo::delete_staged_new_file`]) via `via`, keeping the working
/// copy's bytes and the staged entry first.
pub fn delete_staged_saving(workdir: &Path, path: &str, via: DeleteVia, t: Trashers) -> Result<DeletedSaving> {
    let staged = repo::staged_new(workdir, path)?;
    let (before, too_large) = keep(workdir, path, MAX_KEPT_BYTES)?;
    let how = repo::delete_staged_new_file(workdir, path, t.removal(via))?;
    Ok(finish(path, how, before, via, Some(staged), too_large))
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
    Ok(Plan::PutBack { path: path.clone(), content: *before, staged: staged.clone() })
}

/// Writes `content` back to `path` (which must be absent) with the executable bit unless the
/// repository ignores it (`core.fileMode=false`), and puts `staged` back in the index.
pub fn put_back(workdir: &Path, path: &str, content: Saved, staged: Option<&StagedNew>) -> Result<()> {
    let (bytes, file_mode) = {
        let git = Repository::open(workdir)?;
        let bytes = git.find_blob(content.blob)?.content().to_vec();
        (bytes, git.config().and_then(|c| c.get_bool("core.fileMode")).unwrap_or(true))
    };
    repo::undelete_file(workdir, path, &bytes, file_mode.then_some(content.executable), staged)
}

/// Deletes `path` again via `via`, unstaging it too when it was `staged` as new.
pub fn delete_again(workdir: &Path, path: &str, via: DeleteVia, staged: bool, t: Trashers) -> Result<Deleted> {
    if staged { repo::delete_staged_new_file(workdir, path, t.removal(via)) } else { repo::delete_file(workdir, path, t.removal(via)) }
}
