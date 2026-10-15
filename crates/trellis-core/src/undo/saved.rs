//! A safety net for discards that don't go through a stash (one file, hunk or line) and for
//! restoring a file from a commit: the working copy's exact bytes are written to the object
//! database as a blob before and after, so undo can put the earlier version back and redo can
//! write the later one again.

use super::{Action, Plan, stale};
use crate::error::{Error, Result};
use crate::repo::RestoredFrom;
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
fn current(workdir: &Path, path: &str) -> Result<Option<Saved>> {
    Ok(match read(workdir, path)? {
        Some((bytes, executable)) => Some(Saved { blob: Oid::hash_object(ObjectType::Blob, &bytes)?, executable }),
        None => None,
    })
}

/// The working copy before and after an operation, when the two differ.
type Pair = (Option<Saved>, Option<Saved>);

/// Runs `op` with `path`'s contents saved before and after it. The pair comes back only when
/// they differ, and not at all when the file before isn't one whose contents can be saved (a
/// symbolic link or a folder: `op` still runs). Any other failure to save first refuses `op`.
/// A failure to save afterwards is returned when `strict_after` is set, and otherwise just
/// leaves the operation without a pair.
fn saving<T>(workdir: &Path, path: &str, strict_after: bool, op: impl FnOnce() -> Result<T>) -> Result<(T, Option<Pair>)> {
    let before = match save(workdir, path) {
        Ok(b) => Some(b),
        Err(Error::Unsupported(_)) => None,
        Err(e) => return Err(e),
    };
    let out = op()?;
    let Some(before) = before else { return Ok((out, None)) };
    let after = match save(workdir, path) {
        Ok(a) => a,
        Err(_) if !strict_after => return Ok((out, None)),
        Err(e) => return Err(e),
    };
    Ok((out, (before != after).then_some((before, after))))
}

/// Runs `discard` on `path` with its contents saved first. Returns the action that undoes it,
/// or `None` when nothing changed or the file isn't one whose contents can be saved (a symbolic
/// link or a folder: the discard still runs). Any other failure to save refuses the discard.
pub fn discard_saving(workdir: &Path, path: &str, discard: impl FnOnce() -> Result<()>) -> Result<Option<Action>> {
    let ((), pair) = saving(workdir, path, true, discard)?;
    Ok(pair.map(|(before, after)| Action::Discard { path: path.to_owned(), before, after }))
}

/// Restores `path` from commit `oid` (see [`crate::repo::restore_file`]) with the working copy
/// saved first. Returns where the contents came from and the action that undoes the restore, or
/// `None` when nothing changed or the file before or after isn't one whose contents can be
/// saved (the restore still runs). Errors from the restore, such as [`Error::WouldOverwrite`],
/// come back unchanged; any other failure to save first refuses the restore.
pub fn restore_saving(workdir: &Path, oid: Oid, path: &str, overwrite: bool) -> Result<(RestoredFrom, Option<Action>)> {
    let (from, pair) = saving(workdir, path, false, || crate::repo::restore_file(workdir, oid, path, overwrite))?;
    Ok((from, pair.map(|(before, after)| Action::Restore { path: path.to_owned(), oid, before, after })))
}

impl Action {
    /// The output line for carrying out this action's planned file write, as an undo or a redo.
    /// Empty for actions that never plan a write.
    pub fn wrote(&self, redo: bool) -> String {
        match self {
            Action::Discard { path, .. } if redo => format!("Discarded the changes in {path}"),
            Action::Discard { path, .. } => format!("Restored {path}"),
            Action::Restore { path, .. } if redo => format!("Restored {path} again"),
            Action::Restore { path, before: Some(_), .. } => format!("Put back the earlier working copy of {path}"),
            Action::Restore { path, before: None, .. } => format!("Removed {path} again"),
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

/// Makes `path` hold `content` exactly (deleting it for `None`).
pub fn write(workdir: &Path, path: &str, content: Option<Saved>) -> Result<()> {
    let f = file(workdir, path)?;
    let Some(saved) = content else {
        return match std::fs::remove_file(&f) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    };
    let repo = Repository::open(workdir)?;
    let blob = repo.find_blob(saved.blob)?;
    if let Some(parent) = f.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&f, blob.content())?;
    set_executable(&f, saved.executable)
}

#[cfg(unix)]
fn set_executable(f: &Path, on: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(f)?.permissions();
    let mode = perms.mode();
    perms.set_mode(if on { mode | ((mode & 0o444) >> 2) } else { mode & !0o111 });
    Ok(std::fs::set_permissions(f, perms)?)
}

#[cfg(not(unix))]
fn set_executable(_: &Path, _: bool) -> Result<()> {
    Ok(())
}
