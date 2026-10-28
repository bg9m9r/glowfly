//! Undo and redo of conflict changes made without the conflict view's editor: Mark resolved
//! (`git add`), Mark conflicted (`git checkout -m`) and taking one side's whole version or its
//! deletion (binary, deleted, link and submodule conflicts). Each file's index entries (the
//! conflict's stages 1 to 3, or its stage-0 entry, or none) and what is on disk at its path are
//! recorded before and after the change. Undo puts the "before" back exactly (`marked_put.rs`,
//! through libgit2, so a submodule or modify/delete conflict, which git's resolve-undo record
//! can't bring back, comes back too); redo puts the "after" back. Both run only while HEAD and
//! the operation in progress are as they were and every file still is as the change (or the
//! undo) left it; otherwise the entry is refused as stale and dropped alone. Whatever file bytes
//! a change, an undo or a redo writes over are kept for the journal first (`kept.rs`).

use super::kept::{self, KeptWhat};
use super::planning::busy_name;
use super::record::head;
use super::{Action, MAX_KEPT_BYTES, Plan, Refusal, Saved, Staged, saved};
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

/// A path's index entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexAt {
    /// Conflicted: the `[base, ours, theirs]` stages.
    Conflict([Option<Staged>; 3]),
    /// Not conflicted: its stage-0 entry, `None` when the index has none.
    Merged(Option<Staged>),
}

/// What is on disk at a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkAt {
    Absent,
    /// A regular file's exact bytes. Its blob may be missing from the object database for a
    /// file that was only hashed (one over `MAX_KEPT_BYTES`, or one Mark resolved never writes).
    File(Saved),
    /// A symbolic link, its target as a blob.
    Link(Oid),
    /// A folder (a submodule's checkout), which is never written.
    Folder,
}

impl WorkAt {
    /// Whether the path holds the same thing (a file's executable bit aside).
    pub(super) fn same(&self, other: &WorkAt) -> bool {
        match (self, other) {
            (WorkAt::File(a), WorkAt::File(b)) => a.blob == b.blob,
            (a, b) => a == b,
        }
    }
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

/// `path`'s entries in `index`.
pub(super) fn index_at(index: &Index, path: &str) -> IndexAt {
    let p = Path::new(path);
    let stage = |s: i32| index.get_path(p, s).map(|e| Staged { mode: e.mode, blob: e.id });
    let stages = [stage(1), stage(2), stage(3)];
    if stages.iter().any(Option::is_some) { IndexAt::Conflict(stages) } else { IndexAt::Merged(stage(0)) }
}

/// What is at `path` now. With `save`, a file's bytes (up to `MAX_KEPT_BYTES`) and a link's
/// target are written to the object database; otherwise they are only hashed.
pub(super) fn work_at(workdir: &Path, path: &str, save: bool) -> Result<WorkAt> {
    let full = workdir.join(crate::repo::contained(path)?);
    let meta = match full.symlink_metadata() {
        Ok(m) => m,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => return Ok(WorkAt::Absent),
        Err(e) => return Err(e.into()),
    };
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(&full)?.to_string_lossy().replace('\\', "/");
        let blob =
            if save { Repository::open(workdir)?.blob(target.as_bytes())? } else { Oid::hash_object(ObjectType::Blob, target.as_bytes())? };
        return Ok(WorkAt::Link(blob));
    }
    if meta.is_dir() {
        return Ok(WorkAt::Folder);
    }
    if save && meta.len() <= MAX_KEPT_BYTES {
        return Ok(saved::save(workdir, path)?.map_or(WorkAt::Absent, WorkAt::File));
    }
    let executable = saved::executable(&meta);
    Ok(WorkAt::File(Saved { blob: Oid::hash_file(ObjectType::Blob, &full)?, executable }))
}

/// Each of `paths` as it is now.
fn states(workdir: &Path, paths: &[String], save: bool) -> Result<Vec<PathState>> {
    let repo = Repository::open(workdir)?;
    let index = repo.index()?;
    paths.iter().map(|p| Ok(PathState { index: index_at(&index, p), work: work_at(workdir, p, save)? })).collect()
}

/// Runs `op` (the conflict change `change` of `paths`) with each path's index entries and
/// working copy recorded before and after it. Returns the action that undoes it, or `None`
/// when it changed nothing. A change that writes over files (all but Mark resolved) keeps each
/// file's bytes for the journal first, a failure to keep them refusing it; a record whose file
/// still holds those bytes afterwards is dropped again.
pub fn conflicts_saving(
    workdir: &Path,
    change: ConflictChange,
    paths: &[String],
    op: impl FnOnce() -> Result<()>,
) -> Result<Option<Action>> {
    let writes = change != ConflictChange::MarkResolved;
    let (at, state) = {
        let repo = Repository::open(workdir)?;
        (head(&repo).1, repo.state())
    };
    let before = states(workdir, paths, writes)?;
    let mut records = Vec::new();
    if writes {
        let in_odb = |blob: Oid| Repository::open(workdir).is_ok_and(|r| r.find_blob(blob).is_ok());
        for (p, s) in paths.iter().zip(&before) {
            if let WorkAt::File(saved) = s.work
                && in_odb(saved.blob)
            {
                match kept::keep_file(workdir, p, saved, KeptWhat::Conflict) {
                    Ok(id) => records.push((p.as_str(), id, saved.blob)),
                    Err(e) => {
                        settle(workdir, &records);
                        return Err(e);
                    }
                }
            }
        }
    }
    let out = op();
    settle(workdir, &records);
    out?;
    let after = states(workdir, paths, writes)?;
    let files: Vec<ConflictFile> = paths
        .iter()
        .zip(before.into_iter().zip(after))
        .filter(|(_, (b, a))| b.index != a.index || !b.work.same(&a.work))
        .map(|(p, (before, after))| ConflictFile { path: p.clone(), before, after })
        .collect();
    Ok((!files.is_empty()).then_some(Action::Conflicts { change, head: at, state, files }))
}

/// Drops each record whose file still holds the bytes it kept: nothing was written over.
pub(super) fn settle(workdir: &Path, records: &[(&str, Oid, Oid)]) {
    for (path, id, blob) in records {
        if matches!(work_at(workdir, path, false), Ok(WorkAt::File(s)) if s.blob == *blob) {
            kept::forget(workdir, *id);
        }
    }
}

fn expired(m: impl std::fmt::Display) -> Plan {
    Plan::Refuse(Refusal::Stale(format!("{m}; removed from the undo history")))
}

/// Plans undoing (or redoing) a `Conflicts` action: refused as stale, with nothing written,
/// when HEAD moved, the operation in progress ended (or another began), or any of its files
/// isn't as the change (or the undo) left it, in the index or on disk.
pub(super) fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    let Action::Conflicts { head: at, state, files, .. } = action else { unreachable!("a conflict change") };
    let what = action.describe();
    let repo = Repository::open(workdir)?;
    if head(&repo).1 != *at {
        return Ok(expired(format!("HEAD has moved since the {what}")));
    }
    let now = repo.state();
    if now != *state {
        return Ok(expired(match (busy_name(*state), busy_name(now)) {
            (Some(op), _) => format!("The {op} that was in progress during the {what} is over"),
            (None, Some(op)) => format!("A {op} has started since the {what}"),
            (None, None) => format!("The repository has changed since the {what}"),
        }));
    }
    let index = repo.index()?;
    for f in files {
        let (expect, target) = if redo { (&f.before, &f.after) } else { (&f.after, &f.before) };
        if index_at(&index, &f.path) != expect.index {
            return Ok(expired(format!("{} has changed in the index since", f.path)));
        }
        let work = work_at(workdir, &f.path, false)?;
        if !work.same(&expect.work) {
            return Ok(expired(format!("{} has changed since", f.path)));
        }
        if work.same(&target.work) {
            continue;
        }
        let missing = match target.work {
            WorkAt::File(s) => repo.find_blob(s.blob).is_err(),
            WorkAt::Link(t) => super::marked_put::link_stage(&target.index, t).is_none(),
            WorkAt::Absent => false,
            WorkAt::Folder => true,
        };
        if missing || work == WorkAt::Folder {
            let p = &f.path;
            return Ok(Plan::Refuse(Refusal::Unsafe(format!("Glowfly kept no copy of what {p} held, so it can't be put back"))));
        }
    }
    Ok(Plan::PutConflicts)
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
    let deleted = files.iter().all(|f| f.after.index == IndexAt::Merged(None));
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
