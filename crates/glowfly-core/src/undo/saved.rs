//! A safety net for discards that don't go through a stash (one file, hunk or line) and reverts
//! of part of a commit (`reverted.rs`), for
//! restoring a file from a commit and for conflict resolutions saved in the conflict view
//! (`resolved.rs`): the working copy's exact bytes are written to the object database as a blob
//! before and after, so undo can put the earlier version back and redo can write the later one
//! again.

use super::{Action, DeleteVia, Discarded, Plan, stale};
use crate::error::{Error, Result};
use crate::persist::WriteOptions;
use crate::repo::{RestoredFrom, TRASH_NAME};
use git2::{ObjectType, Oid, Repository};
use std::path::{Path, PathBuf};

/// A working-tree file's exact bytes (no filters applied), kept as a blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    pub blob: Oid,
    /// The executable bit, on systems that have one.
    pub executable: bool,
}

fn file(workdir: &Path, path: &str) -> Result<PathBuf> {
    Ok(workdir.join(crate::repo::contained(path)?))
}

/// Reads `path` as it is: `None` when it is absent; refused for anything but a regular file.
fn read(workdir: &Path, path: &str) -> Result<Option<(Vec<u8>, bool)>> {
    let f = file(workdir, path)?;
    let meta = match f.symlink_metadata() {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !meta.is_file() {
        return Err(Error::Unsupported(format!("{path} is not a regular file, so its contents can't be saved for undo")));
    }
    Ok(Some((std::fs::read(&f)?, executable(&meta))))
}

#[cfg(unix)]
fn executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    false
}

/// Writes `path`'s current bytes to the object database (`None` when it is absent).
pub fn save(workdir: &Path, path: &str) -> Result<Option<Saved>> {
    let Some((bytes, executable)) = read(workdir, path)? else { return Ok(None) };
    let repo = Repository::open(workdir)?;
    Ok(Some(Saved { blob: repo.blob(&bytes)?, executable }))
}

/// What `path` holds now, as `save` would record it, without writing anything.
pub(super) fn current(workdir: &Path, path: &str) -> Result<Option<Saved>> {
    Ok(match read(workdir, path)? {
        Some((bytes, executable)) => Some(Saved { blob: Oid::hash_object(ObjectType::Blob, &bytes)?, executable }),
        None => None,
    })
}

/// The working copy before and after an operation, when the two differ.
type Pair = (Option<Saved>, Option<Saved>);

/// A restore (also a revert of part of a commit, or a delete) that failed. `undo` is set when the file changed anyway (git removes a file before
/// a failing required filter runs, a write can be cut short), so the working copy it held can
/// still be put back; the error then names the blob that keeps it.
#[derive(Debug)]
pub struct RestoreFailed {
    pub error: Error,
    pub undo: Option<Box<Action>>,
}

/// Runs `op` with `path`'s contents saved before and after it. The pair comes back only when
/// they differ, and not at all when the file before isn't one whose contents can be saved (a
/// symbolic link or a folder: `op` still runs). Any other failure to save first refuses `op`.
/// A failure to save afterwards is returned when `strict_after` is set, and otherwise just
/// leaves the operation without a pair. When `op` fails but the file changed anyway (or can no
/// longer be read), its error names the blob that keeps the earlier contents, and the pair
/// comes back with it when the file as it is now could be saved.
pub(super) fn saving<T>(
    workdir: &Path,
    path: &str,
    strict_after: bool,
    op: impl FnOnce() -> Result<T>,
) -> std::result::Result<(T, Option<Pair>), (Error, Option<Pair>)> {
    let before = match save(workdir, path) {
        Ok(b) => Some(b),
        Err(Error::Unsupported(_)) => None,
        Err(e) => return Err((e, None)),
    };
    let out = match op() {
        Ok(out) => out,
        Err(e) => {
            return Err(match before {
                Some(before) => failed_after_saving(workdir, path, before, e),
                None => (e, None),
            });
        }
    };
    let Some(before) = before else { return Ok((out, None)) };
    let after = match save(workdir, path) {
        Ok(a) => a,
        Err(_) if !strict_after => return Ok((out, None)),
        Err(e) => return Err((e, None)),
    };
    Ok((out, (before != after).then_some((before, after))))
}

/// `op` failed after `path` was saved as `before`. Unchanged, the error comes back as it was;
/// otherwise it says where the earlier contents are kept, with the pair when the file as it is
/// now could be saved too.
pub(super) fn failed_after_saving(workdir: &Path, path: &str, before: Option<Saved>, e: Error) -> (Error, Option<Pair>) {
    let after = save(workdir, path);
    if after.as_ref().is_ok_and(|a| *a == before) {
        return (e, None);
    }
    let error = match before {
        Some(s) => Error::Other(format!(
            "{e}. {path} changed anyway; its earlier contents are kept as blob {} (git cat-file -p {})",
            s.blob, s.blob
        )),
        None => e,
    };
    (error, after.ok().map(|a| (before, a)))
}

/// Runs `discard` on `path` (`what` says which part) with its contents saved first. Returns the action that undoes it,
/// or `None` when nothing changed or the file isn't one whose contents can be saved (a symbolic
/// link or a folder: the discard still runs). Any other failure to save refuses the discard. A
/// discard that fails after changing the file says where its earlier contents are kept.
pub fn discard_saving(workdir: &Path, path: &str, what: Discarded, discard: impl FnOnce() -> Result<()>) -> Result<Option<Action>> {
    let ((), pair) = saving(workdir, path, true, discard).map_err(|(e, _)| e)?;
    Ok(pair.map(|(before, after)| Action::Discard { path: path.to_owned(), what, before, after }))
}

/// Restores `path` from commit `oid` (see [`crate::repo::restore_file`]) with the working copy
/// saved first. Returns where the contents came from and the action that undoes the restore, or
/// `None` when nothing changed or the file before or after isn't one whose contents can be
/// saved (the restore still runs). Errors from the restore, such as [`Error::WouldOverwrite`],
/// come back unchanged unless the file changed anyway, when they come with the action that puts
/// it back; any other failure to save first refuses the restore.
pub fn restore_saving(
    workdir: &Path,
    oid: Oid,
    path: &str,
    overwrite: bool,
) -> std::result::Result<(RestoredFrom, Option<Action>), RestoreFailed> {
    let action = |(before, after): Pair| Action::Restore { path: path.to_owned(), oid, before, after };
    match saving(workdir, path, false, || crate::repo::restore_file(workdir, oid, path, overwrite)) {
        Ok((from, pair)) => Ok((from, pair.map(action))),
        Err((error, pair)) => Err(RestoreFailed { error, undo: pair.map(|p| Box::new(action(p))) }),
    }
}

impl Action {
    /// The output line for carrying out this action's planned file write, as an undo or a redo.
    /// Empty for actions that never plan a write.
    pub fn wrote(&self, redo: bool) -> String {
        match self {
            Action::Discard { path, what, .. } if redo => match what {
                Discarded::File => format!("Discarded the changes in {path}"),
                Discarded::Hunk => format!("Discarded the hunk in {path} again"),
                Discarded::Lines(_) => format!("Discarded the lines in {path} again"),
                Discarded::RevertedHunk => format!("Reverted the hunk in {path} again"),
                Discarded::RevertedLines(_) => format!("Reverted the lines in {path} again"),
                Discarded::RevertedFile => format!("Reverted the changes to {path} again"),
            },
            Action::Discard { path, .. } => format!("Restored {path}"),
            Action::Restore { path, .. } if redo => format!("Restored {path} again"),
            Action::Restore { path, before: Some(_), .. } => format!("Put back the earlier working copy of {path}"),
            Action::Restore { path, before: None, .. } => format!("Removed {path} again"),
            Action::Resolve { path, staged, .. } => super::resolved::wrote(path, staged.is_some(), redo),
            Action::DeleteFile { path, via, .. } if redo => match via {
                DeleteVia::Trash => format!("Moved {path} to the {TRASH_NAME} again"),
                DeleteVia::HomeTrash => format!("Moved {path} to the Trash in your home folder again"),
                DeleteVia::Permanent => format!("Deleted {path} again"),
            },
            // How it came back (out of the trash, or from the kept copy) is `PutBackHow::wrote`.
            Action::DeleteFile { path, via, staged, .. } => super::PutBackHow::Written { trash: None }.wrote(path, staged.is_some(), *via),
            Action::UnstageGone { path, .. } if redo => format!("Removed {path} from the staging area again"),
            Action::UnstageGone { path, .. } => format!("Staged {path} again"),
            _ => String::new(),
        }
    }
}

/// Undo puts `before` back over `after`; redo the reverse. Refused as stale when the file no
/// longer holds what the operation (or the undo) left, since writing would lose those edits.
/// `kept_as` names the earlier version in that refusal ("discarded", "overwritten").
pub(super) fn plan(workdir: &Path, path: &str, before: Option<Saved>, after: Option<Saved>, redo: bool, kept_as: &str) -> Result<Plan> {
    let (expect, target) = if redo { (before, after) } else { (after, before) };
    let now = match current(workdir, path) {
        Ok(n) => n,
        Err(Error::Unsupported(m)) => return Ok(stale(m)),
        Err(e) => return Err(e),
    };
    if now.map(|s| s.blob) != expect.map(|s| s.blob) {
        let kept = match before {
            Some(s) if !redo => format!("; the {kept_as} version is kept as blob {} (git cat-file -p {})", s.blob, s.blob),
            _ => String::new(),
        };
        return Ok(stale(format!("{path} has changed since{kept}")));
    }
    Ok(Plan::Write { path: path.to_owned(), content: target })
}

/// Makes `path` hold `content` exactly (deleting it for `None`). The file is replaced
/// crash-safely, keeping its permissions (and a read-only attribute); a symbolic link there is
/// refused rather than written through. The executable bit is set as saved unless the
/// repository ignores it (`core.fileMode=false`), and failing to set it doesn't fail the write.
pub fn write(workdir: &Path, path: &str, content: Option<Saved>) -> Result<()> {
    let f = file(workdir, path)?;
    let Some(saved) = content else {
        return Ok(crate::persist::remove_file(&f)?);
    };
    let repo = Repository::open(workdir)?;
    let blob = repo.find_blob(saved.blob)?;
    let file_mode = repo.config().and_then(|c| c.get_bool("core.fileMode")).unwrap_or(true);
    let opts = WriteOptions { refuse_links: true, executable: file_mode.then_some(saved.executable), private: false };
    Ok(crate::persist::write_file(&f, blob.content(), opts)?)
}
