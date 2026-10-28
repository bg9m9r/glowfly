//! Undo and redo of conflict changes made without the conflict view's editor: Mark resolved
//! (`git add`), Mark conflicted (`git checkout -m`) and taking one side's whole version or its
//! deletion (binary, deleted, link and submodule conflicts). Each file's index entries (the
//! conflict's stages 1 to 3, or its stage-0 entry, or none) and what is on disk at its path are
//! recorded before and after the change, with the commits the operation in progress names
//! (MERGE_HEAD and the like). Undo puts the "before" back exactly (`marked_put.rs`,
//! through libgit2, so a submodule or modify/delete conflict, which git's resolve-undo record
//! can't bring back, comes back too); redo puts the "after" back. Both run only while HEAD and
//! the operation in progress are as they were and every file still is as the change (or the
//! undo) left it (`marked_plan.rs`); otherwise the entry is refused and dropped alone. Whatever file bytes
//! a change, an undo or a redo writes over are kept for the journal first (`kept.rs`).

use super::aborted::KEPT_FLAGS;
use super::kept::{self, KeptWhat};
use super::marked_plan::{check_paths, op_heads, restorable};
use super::record::head;
use super::{Action, MAX_KEPT_BYTES, Saved, Staged, saved};
use crate::error::{Error, Result};
use crate::repo::Side;
use git2::{Index, ObjectType, Oid, Repository};
use std::path::Path;

/// What changed a set of conflicted files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictChange {
    /// Mark resolved: each file staged as it is.
    MarkResolved,
    /// Mark conflicted: each conflict brought back from git's resolve-undo record.
    MarkConflicted,
    /// One side's whole version taken (or its deletion, when that side deleted the file).
    Took(Side),
}

/// A stage-0 entry's kept flags: assume-unchanged (`KEPT_FLAGS` of `flags`) and the extended
/// flags (skip-worktree, intent-to-add).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryFlags {
    pub flags: u16,
    pub extended: u16,
}

/// A path's index entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexAt {
    /// Conflicted: the `[base, ours, theirs]` stages.
    Conflict([Option<Staged>; 3]),
    /// Not conflicted: its stage-0 entry with its flags, `None` when the index has none.
    Merged(Option<Staged>, EntryFlags),
}

/// What is on disk at a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkAt {
    Absent,
    /// A regular file's exact bytes and executable bit. Its blob may be missing from the object
    /// database for a file that was only hashed (one Mark resolved never writes over).
    File(Saved),
    /// A symbolic link, its target as a blob.
    Link(Oid),
    /// A folder (a submodule's checkout), which is never written.
    Folder,
}

/// A path's index entries and what is on disk there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathState {
    pub index: IndexAt,
    pub work: WorkAt,
}

/// One file a conflict change changed, as it was before and after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictFile {
    pub path: String,
    pub before: PathState,
    pub after: PathState,
}

/// A conflict change that failed. `undo` is set when it changed some of its files anyway (a
/// later batch of a Mark resolved, say), so those can still be put back.
#[derive(Debug)]
pub struct ConflictsFailed {
    pub error: Error,
    pub undo: Option<Action>,
}

impl From<Error> for ConflictsFailed {
    fn from(error: Error) -> Self {
        Self { error, undo: None }
    }
}

/// Whether `index` holds a stage-0 entry for `path` beside conflict stages, which `IndexAt`
/// can't hold: such an index is refused rather than have the stage-0 entry dropped.
pub(super) fn mixed(index: &Index, path: &str) -> bool {
    let p = Path::new(path);
    index.get_path(p, 0).is_some() && (1..=3).any(|s| index.get_path(p, s).is_some())
}

/// `path`'s entries in `index`.
pub(super) fn index_at(index: &Index, path: &str) -> IndexAt {
    let p = Path::new(path);
    let stage = |s: i32| index.get_path(p, s).map(|e| Staged { mode: e.mode, blob: e.id });
    let stages = [stage(1), stage(2), stage(3)];
    if stages.iter().any(Option::is_some) {
        return IndexAt::Conflict(stages);
    }
    let flags = index.get_path(p, 0).map(|e| EntryFlags { flags: e.flags & KEPT_FLAGS, extended: e.flags_extended });
    IndexAt::Merged(stage(0), flags.unwrap_or_default())
}

/// What is at `path` now. With `save`, a file's bytes (up to `MAX_KEPT_BYTES`) and a link's
/// target are written to the object database; otherwise they are only hashed.
pub(super) fn work_at(workdir: &Path, path: &str, save: bool) -> Result<WorkAt> {
    work_kept(workdir, path, save.then_some(MAX_KEPT_BYTES))
}

/// `work_at`, saving a file of up to `limit` bytes (none without one).
fn work_kept(workdir: &Path, path: &str, limit: Option<u64>) -> Result<WorkAt> {
    let full = workdir.join(crate::repo::contained(path)?);
    let meta = match full.symlink_metadata() {
        Ok(m) => m,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => return Ok(WorkAt::Absent),
        Err(e) => return Err(e.into()),
    };
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(&full)?.to_string_lossy().replace('\\', "/");
        let blob = if limit.is_some() {
            Repository::open(workdir)?.blob(target.as_bytes())?
        } else {
            Oid::hash_object(ObjectType::Blob, target.as_bytes())?
        };
        return Ok(WorkAt::Link(blob));
    }
    if meta.is_dir() {
        return Ok(WorkAt::Folder);
    }
    if limit.is_some_and(|l| meta.len() <= l) {
        return Ok(saved::save(workdir, path)?.map_or(WorkAt::Absent, WorkAt::File));
    }
    let executable = saved::executable(&meta);
    Ok(WorkAt::File(Saved { blob: Oid::hash_file(ObjectType::Blob, &full)?, executable }))
}

/// Each of `paths` as it is now; refused for an index holding a stage-0 entry beside a conflict.
fn states(workdir: &Path, paths: &[String], limit: Option<u64>) -> Result<Vec<PathState>> {
    let repo = Repository::open(workdir)?;
    let index = repo.index()?;
    let read = |p: &String| {
        if mixed(&index, p) {
            return Err(Error::Unsupported(format!(
                "{p} has a resolved entry beside its conflict in the index; resolve it with git first"
            )));
        }
        Ok(PathState { index: index_at(&index, p), work: work_kept(workdir, p, limit)? })
    };
    paths.iter().map(read).collect()
}

/// Runs `op` (the conflict change `change` of `paths`) with each path's index entries and
/// working copy recorded before and after it. Returns the action that undoes it, or `None`
/// when it changed nothing. A change that writes over files (all but Mark resolved) keeps each
/// file's bytes for the journal first and is refused, with nothing changed, for a file it
/// can't keep (over `MAX_KEPT_BYTES`) or couldn't put back (a link no index entry holds); a
/// record whose file still holds those bytes afterwards is dropped again. A change that failed
/// partway still returns the action for the files it changed, with its error.
pub fn conflicts_saving(
    workdir: &Path,
    change: ConflictChange,
    paths: &[String],
    op: impl FnOnce() -> Result<()>,
) -> std::result::Result<Option<Action>, ConflictsFailed> {
    conflicts_saving_with(workdir, change, paths, MAX_KEPT_BYTES, op)
}

/// `conflicts_saving`, keeping files of up to `limit` bytes.
pub(super) fn conflicts_saving_with(
    workdir: &Path,
    change: ConflictChange,
    paths: &[String],
    limit: u64,
    op: impl FnOnce() -> Result<()>,
) -> std::result::Result<Option<Action>, ConflictsFailed> {
    let writes = change != ConflictChange::MarkResolved;
    let keep = writes.then_some(limit);
    let (at, state, heads) = {
        let repo = Repository::open(workdir).map_err(Error::from)?;
        check_paths(&repo, paths)?;
        (head(&repo).1, repo.state(), op_heads(&repo))
    };
    let before = states(workdir, paths, keep)?;
    if writes {
        let repo = Repository::open(workdir).map_err(Error::from)?;
        for (p, s) in paths.iter().zip(&before) {
            restorable(&repo, p, s)?;
        }
    }
    let mut records = Vec::new();
    if writes {
        for (p, s) in paths.iter().zip(&before) {
            if let WorkAt::File(saved) = s.work {
                match kept::keep_file(workdir, p, saved, KeptWhat::Conflict) {
                    Ok(id) => records.push((p.as_str(), id, saved.blob)),
                    Err(e) => {
                        settle(workdir, &records);
                        return Err(e.into());
                    }
                }
            }
        }
    }
    let out = op();
    settle(workdir, &records);
    let after = match states(workdir, paths, keep) {
        Ok(a) => a,
        Err(e) => {
            let what = match out {
                Ok(()) => "it can't be undone".to_owned(),
                Err(op) => format!("it can't be undone. It failed: {op}"),
            };
            return Err(Error::Other(format!("Glowfly couldn't read what the change left, so {what} ({e})")).into());
        }
    };
    let files: Vec<ConflictFile> = paths
        .iter()
        .zip(before.into_iter().zip(after))
        .filter(|(_, (b, a))| b != a)
        .map(|(p, (before, after))| ConflictFile { path: p.clone(), before, after })
        .collect();
    let undo = (!files.is_empty()).then_some(Action::Conflicts { change, head: at, state, heads, files });
    match out {
        Ok(()) => Ok(undo),
        Err(error) => Err(ConflictsFailed { error, undo }),
    }
}

/// Drops each record whose file still holds the bytes it kept: nothing was written over.
pub(super) fn settle(workdir: &Path, records: &[(&str, Oid, Oid)]) {
    for (path, id, blob) in records {
        if matches!(work_at(workdir, path, false), Ok(WorkAt::File(s)) if s.blob == *blob) {
            kept::forget(workdir, *id);
        }
    }
}

/// "a.txt" for one path, "3 files" for several.
fn named(files: &[ConflictFile]) -> String {
    match files {
        [f] => f.path.clone(),
        _ => format!("{} files", files.len()),
    }
}

/// What the change did, for tooltips and job labels.
pub(super) fn describe(change: ConflictChange, files: &[ConflictFile]) -> String {
    let n = named(files);
    let deleted = files.iter().all(|f| matches!(f.after.index, IndexAt::Merged(None, _)));
    match change {
        ConflictChange::MarkResolved => format!("marking {n} resolved"),
        ConflictChange::MarkConflicted => format!("marking {n} conflicted"),
        ConflictChange::Took(_) if deleted => format!("deletion of {n} to resolve the conflict"),
        ConflictChange::Took(Side::Ours) => format!("taking our version of {n}"),
        ConflictChange::Took(Side::Theirs) => format!("taking their version of {n}"),
    }
}

/// The output line for carrying out an undo (or redo) of the change.
pub(super) fn wrote(change: ConflictChange, files: &[ConflictFile], redo: bool) -> String {
    let n = named(files);
    let conflicts = if files.len() == 1 { "conflict" } else { "conflicts" };
    match (change, redo) {
        (ConflictChange::MarkConflicted, false) => format!("Put back the resolution of {n}"),
        (ConflictChange::MarkConflicted, true) | (_, false) => format!("Brought back the {conflicts} in {n}"),
        (ConflictChange::MarkResolved, true) => format!("Marked {n} resolved again"),
        (ConflictChange::Took(_), true) => format!("Resolved {n} again with the same version"),
    }
}

/// The error for a path the keep step can't keep.
pub(super) fn too_large(path: &str) -> Error {
    Error::Unsupported(format!("{path} is over 64 MB, so Glowfly can't keep a copy of it before writing over it"))
}
