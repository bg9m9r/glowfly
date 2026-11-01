//! Which file rows hold Git LFS pointers (`FileEntry::lfs`), and what the two sides of a diff
//! are when one of them is a pointer (`ViewDiff::lfs`).
//!
//! Only the stored content counts: the index or a commit's blob, or the working copy where git
//! has nothing stored (an untracked file, the new side of a working-tree comparison). Blobs are
//! sized from their object header and files from their metadata, so nothing large is read.

use super::{DiffTarget, FileEntry, Hazard, rpath};
use crate::lfs::pointer::{self, LfsChange, LfsSide, MAX_POINTER_BYTES};
use git2::{Delta, Diff, DiffDelta, DiffFile, FileMode, Index, ObjectType, Oid, Repository, Tree};
use std::path::Path;

/// The most rows of one list checked; rows past it are left unmarked.
const MAX_LFS_CHECKS: usize = 2000;

/// A gitlink's mode in the index: a submodule's commit, never a pointer.
const GITLINK: u32 = 0o160000;

/// Whether a row can be checked at all: conflicted rows and names that aren't UTF-8 never are.
fn checkable(f: &FileEntry) -> bool {
    f.status != '!' && !matches!(f.hazard, Some(Hazard::NotUtf8(_)))
}

/// The index's blob for `path`, when it has a regular stage-0 entry.
fn index_blob(index: &Index, path: &str) -> Option<Oid> {
    index.get_path(Path::new(path), 0).filter(|e| e.mode != GITLINK).map(|e| e.id)
}

fn blob_is_pointer(repo: &Repository, id: Option<Oid>) -> bool {
    id.is_some_and(|id| pointer::read_blob(repo, id).is_some())
}

fn file_is_pointer(workdir: Option<&Path>, path: &str) -> bool {
    workdir.is_some_and(|w| pointer::read_file(&w.join(path)).is_some())
}

/// Marks a whole status read's rows: staged rows by the index blob (HEAD's for a staged
/// deletion), unstaged ones as `mark_unstaged` does.
pub(super) fn mark_wip(repo: &Repository, unstaged: &mut [FileEntry], staged: &mut [FileEntry]) {
    let Ok(index) = repo.index() else { return };
    mark_staged(repo, &index, staged);
    mark_unstaged(repo, &index, unstaged);
}

/// Marks staged rows by the index blob (HEAD's for a staged deletion).
pub(super) fn mark_staged(repo: &Repository, index: &Index, staged: &mut [FileEntry]) {
    let mut head: Option<Option<Tree<'_>>> = None;
    for f in staged.iter_mut().take(MAX_LFS_CHECKS).filter(|f| checkable(f)) {
        let id = if f.status == 'D' {
            let tree = head.get_or_insert_with(|| super::diff::head_tree(repo));
            tree.as_ref().and_then(|t| t.get_path(Path::new(&f.path)).ok()).filter(|e| e.kind() == Some(ObjectType::Blob)).map(|e| e.id())
        } else {
            index_blob(index, &f.path)
        };
        f.lfs = blob_is_pointer(repo, id);
    }
}

/// Marks unstaged rows: the index blob or the working copy is a pointer (only the working copy
/// for an untracked file).
pub(super) fn mark_unstaged(repo: &Repository, index: &Index, rows: &mut [FileEntry]) {
    let workdir = repo.workdir();
    for f in rows.iter_mut().take(MAX_LFS_CHECKS).filter(|f| checkable(f)) {
        let entry = index.get_path(Path::new(&f.path), 0);
        if entry.as_ref().is_some_and(|e| e.mode == GITLINK) {
            continue;
        }
        let indexed = entry.filter(|_| f.status != '?').map(|e| e.id);
        f.lfs = blob_is_pointer(repo, indexed) || file_is_pointer(workdir, &f.path);
    }
}

/// Marks a diff's rows, `files` being one entry per delta in delta order (as `diff::file_entries`
/// builds them): by the new side, or the old one for a deletion; with `new_in_workdir`, the new
/// side is the working copy.
pub(super) fn mark_delta_files(repo: &Repository, diff: &Diff, files: &mut [FileEntry], new_in_workdir: bool) {
    let workdir = repo.workdir();
    for (d, f) in diff.deltas().zip(files.iter_mut()).take(MAX_LFS_CHECKS) {
        if !checkable(f) {
            continue;
        }
        f.lfs = if d.status() == Delta::Deleted {
            regular_blob(&d.old_file()).is_some_and(|id| pointer::read_blob(repo, id).is_some())
        } else if new_in_workdir {
            d.new_file().mode() != FileMode::Commit && file_is_pointer(workdir, &f.path)
        } else {
            regular_blob(&d.new_file()).is_some_and(|id| pointer::read_blob(repo, id).is_some())
        };
    }
}

/// Marks the rows of a working-tree-to-commit diff (`mark_delta_files` with `new_in_workdir`,
/// the other way round), so a row's mark doesn't change when the comparison is swapped: the
/// commit's blob when the working tree lacks the file ('A'), else the working copy.
pub(super) fn mark_reversed_workdir_files(repo: &Repository, diff: &Diff, files: &mut [FileEntry]) {
    let workdir = repo.workdir();
    for (d, f) in diff.deltas().zip(files.iter_mut()).take(MAX_LFS_CHECKS) {
        if !checkable(f) {
            continue;
        }
        f.lfs = if d.status() == Delta::Added {
            blob_is_pointer(repo, regular_blob(&d.new_file()))
        } else if d.old_file().mode() == FileMode::Commit {
            false
        } else {
            let path = d.old_file().path_bytes().and_then(rpath::text).unwrap_or_else(|| f.path.clone());
            file_is_pointer(workdir, &path)
        };
    }
}

/// Whether either side of `delta` is a Git LFS pointer: the old blob, or the new blob (the
/// working copy with `in_workdir`).
pub(super) fn delta_is_pointer(repo: &Repository, delta: &DiffDelta<'_>, in_workdir: bool) -> bool {
    if blob_is_pointer(repo, regular_blob(&delta.old_file())) {
        return true;
    }
    match delta.status() {
        Delta::Deleted => false,
        _ if in_workdir => rpath::text(rpath::delta_bytes(delta)).is_some_and(|p| file_is_pointer(repo.workdir(), &p)),
        _ => blob_is_pointer(repo, regular_blob(&delta.new_file())),
    }
}

/// The blob id of a diff side that is a file (not a gitlink, not absent).
fn regular_blob(file: &DiffFile<'_>) -> Option<Oid> {
    (file.mode() != FileMode::Commit).then(|| file.id()).filter(|id| !id.is_zero())
}

/// The sides of `target`'s file in `diff` when either one is an LFS pointer.
pub(super) fn diff_change(repo: &Repository, target: &DiffTarget, diff: &Diff) -> Option<LfsChange> {
    let path = target.path();
    let d = rpath::find_delta(diff, path)?;
    if d.old_file().mode() == FileMode::Commit || d.new_file().mode() == FileMode::Commit {
        return None;
    }
    let old = match d.status() {
        Delta::Added | Delta::Untracked => LfsSide::Absent,
        _ => blob_side(repo, d.old_file().id()),
    };
    let new_in_workdir = matches!(target, DiffTarget::Unstaged(_) | DiffTarget::ToWorkdir(..));
    let new = match d.status() {
        Delta::Deleted => LfsSide::Absent,
        _ if new_in_workdir => {
            let disk = rpath::text(rpath::delta_bytes(&d)).zip(repo.workdir()).map(|(p, w)| w.join(p));
            disk.map_or(LfsSide::Absent, |p| file_side(&p))
        }
        _ => blob_side(repo, d.new_file().id()),
    };
    let mut change = LfsChange { old, new, new_in_workdir, filtered: false };
    if !change.has_pointer() {
        return None;
    }
    change.filtered = new_in_workdir && super::attrs::lfs_filtered(repo, path);
    Some(change)
}

/// A blob as a diff side: its pointer, or a regular file of the header's size.
fn blob_side(repo: &Repository, id: Oid) -> LfsSide {
    if id.is_zero() {
        return LfsSide::Absent;
    }
    if let Some(p) = pointer::read_blob(repo, id) {
        return LfsSide::Pointer(p);
    }
    match repo.odb().and_then(|odb| odb.read_header(id)) {
        Ok((size, _)) => LfsSide::Regular { size: size as u64 },
        Err(_) => LfsSide::Absent,
    }
}

/// A working-tree file as a diff side: its pointer, or a regular file of its length.
fn file_side(path: &Path) -> LfsSide {
    let Ok(meta) = path.symlink_metadata() else { return LfsSide::Absent };
    if meta.is_file()
        && meta.len() <= MAX_POINTER_BYTES as u64
        && let Some(p) = pointer::read_file(path)
    {
        return LfsSide::Pointer(p);
    }
    LfsSide::Regular { size: meta.len() }
}

#[cfg(test)]
#[path = "lfs_marks_tests.rs"]
mod tests;
