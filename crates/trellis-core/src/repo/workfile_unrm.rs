//! Putting a deleted working-tree file back for undo, and the staged entry of a new file Unstage
//! and delete removed.
//!
//! As in `workfile_rm.rs`, the index is changed with libgit2 and the exact index helpers rather
//! than the git CLI: only a blob already in the object database goes back into the index, so no
//! hooks, config, credentials or filters apply.

use super::head_tree;
use super::workfile::{check_parents, contained, create_parents, ensure_inside, open_root, remove_dirs};
use super::workfile_rm::check_staged_new;
use crate::error::{Error, Result};
use git2::{Index, IndexEntry, IndexTime, Oid, Repository};
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// The stage-0 index entry of a file staged as new (or as a staged rename's new path), kept
/// before Unstage and delete removes it: its blob, its mode and the path the rename came from
/// (git's bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedNew {
    pub blob: Oid,
    pub mode: u32,
    pub source: Option<Vec<u8>>,
}

/// The staged entry of `path`, which must be staged as new or as a rename's new path (refused
/// otherwise, as `delete_staged_new_file` refuses).
pub fn staged_new(workdir: &Path, path: &str) -> Result<StagedNew> {
    let path = contained(path)?;
    let repo = Repository::open(workdir)?;
    let source = check_staged_new(&repo, path)?;
    let entry = super::exact::index(&repo)?.get_path(Path::new(path), 0).ok_or_else(|| Error::Other(format!("{path} isn't staged")))?;
    Ok(StagedNew { blob: entry.id, mode: entry.mode, source })
}

/// The index entry for `path` (git's bytes) at `stage`, compared byte for byte.
fn entry_at(index: &Index, path: &[u8], stage: u16) -> Option<IndexEntry> {
    index.iter().find(|e| e.path == path && (e.flags >> 12) & 0x3 == stage)
}

/// Whether the index is as it was just before Unstage and delete: `path` staged with `s`'s blob
/// and mode, no conflict for it, and the rename's source (if any) staged as deleted.
pub fn staged_new_matches(workdir: &Path, path: &str, s: &StagedNew) -> Result<bool> {
    let repo = Repository::open(workdir)?;
    let index = super::exact::index(&repo)?;
    let p = path.as_bytes();
    let same = entry_at(&index, p, 0).is_some_and(|e| e.id == s.blob && e.mode == s.mode);
    let conflicted = (1..=3).any(|stage| entry_at(&index, p, stage).is_some());
    let source_deleted = s.source.as_deref().is_none_or(|src| entry_at(&index, src, 0).is_none());
    Ok(same && !conflicted && source_deleted)
}

/// Whether the index is as Unstage and delete left it: no entry for `path` at any stage, and the
/// rename's source (if any) absent or back as HEAD has it.
pub fn index_clear_for(workdir: &Path, path: &str, s: &StagedNew) -> Result<bool> {
    let repo = Repository::open(workdir)?;
    let index = super::exact::index(&repo)?;
    if (0..=3).any(|stage| entry_at(&index, path.as_bytes(), stage).is_some()) {
        return Ok(false);
    }
    let Some(src) = s.source.as_deref() else { return Ok(true) };
    Ok(match entry_at(&index, src, 0) {
        None => true,
        Some(e) => head_tree(&repo)
            .and_then(|t| super::exact::tree_entry(&repo, &t, src))
            .is_some_and(|h| h.id() == e.id && u32::try_from(h.filemode()).ok() == Some(e.mode)),
    })
}

/// Writes `bytes` to the working-tree file `path`, which must not exist (a case twin on a
/// case-insensitive volume counts), recreating the parent folders the delete pruned. The
/// executable bit is set when `executable` is `Some(true)` (systems that have one). With
/// `staged`, the index entry goes back too, and a rename's source loses the HEAD entry Unstage
/// and delete gave it, so the staged rename shows again.
///
/// Nothing is ever overwritten: a file that appeared at `path` is refused, changing nothing.
/// When only the index step fails, the file stays back and the error says so.
pub fn undelete_file(workdir: &Path, path: &str, bytes: &[u8], executable: Option<bool>, staged: Option<&StagedNew>) -> Result<()> {
    let rel = contained(path)?;
    let (repo, root) = open_root(workdir)?;
    drop(repo);
    check_parents(&root, rel)?;
    let created = create_parents(&root, rel)?;
    let full = root.join(rel);
    let written = ensure_inside(&root, full.parent().unwrap_or(&root)).and_then(|()| {
        write_new(&full, bytes, executable).map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => Error::Other(format!("{rel} is in your working tree again")),
            _ => e.into(),
        })
    });
    if let Err(e) = written {
        remove_dirs(&created);
        return Err(e);
    }
    if let Some(s) = staged {
        restage(&root, rel, s).map_err(|e| Error::Other(format!("{rel} is back, but it couldn't be staged again: {e}")))?;
    }
    Ok(())
}

/// Creates `full` (never replacing anything) holding `bytes`; a failed write removes it again.
fn write_new(full: &Path, bytes: &[u8], executable: Option<bool>) -> std::io::Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(full)?;
    let r = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    if let Err(e) = r {
        let _ = std::fs::remove_file(full);
        return Err(e);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if executable == Some(true)
            && let Ok(m) = std::fs::metadata(full)
        {
            let mut p = m.permissions();
            let mode = p.mode();
            // Execute for whoever may read it, as git checks an executable file out.
            p.set_mode(mode | ((mode & 0o444) >> 2));
            let _ = std::fs::set_permissions(full, p);
        }
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

/// Puts `s`'s entry back at `path` and takes the rename's source out of the index again, in one
/// index write.
fn restage(workdir: &Path, path: &str, s: &StagedNew) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = super::exact::index(&repo)?;
    let zero = IndexTime::new(0, 0);
    index.add(&IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode: s.mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: s.blob,
        flags: u16::try_from(path.len().min(0xfff)).unwrap_or(0xfff),
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    })?;
    if let Some(src) = s.source.as_deref()
        && entry_at(&index, src, 0).is_some()
    {
        index.remove(&path_of(src)?, 0)?;
    }
    index.write()?;
    Ok(())
}

/// git's path bytes as a `Path`: the raw bytes where the system has them, else the text.
fn path_of(bytes: &[u8]) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
    }
    #[cfg(not(unix))]
    {
        super::rpath::text(bytes)
            .map(PathBuf::from)
            .ok_or_else(|| Error::Unsupported(format!("{} isn't a UTF-8 path", super::rpath::shown(bytes))))
    }
}
