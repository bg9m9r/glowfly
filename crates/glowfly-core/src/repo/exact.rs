//! Exact index edits: one path, byte for byte.
//!
//! Two things in libgit2 match more than the path asked for. `reset_default` and the other
//! pathspec APIs read paths as globs (`[id].tsx` also matches `i.tsx` and `d.tsx`). And the
//! repository's index folds case when `core.ignorecase` is set, so looking up, adding or
//! removing `a.txt` can reach an `A.txt` entry. The index opened here is the same file read
//! on its own, without the repository's config, so it compares paths exactly.

use crate::error::{Error, Result};
use git2::{ApplyOptions, Diff, Index, IndexEntry, IndexTime, ObjectType, Oid, Repository, Tree, TreeEntry};
use std::path::Path;

/// The repository's index file, opened so that paths compare exactly (see the module comment).
/// Changes are saved by `Index::write`, as with `Repository::index`.
pub(super) fn index(repo: &Repository) -> Result<Index> {
    let shared = repo.index()?;
    match shared.path() {
        Some(file) => Ok(Index::open(file)?),
        None => Ok(shared),
    }
}

/// The entry at `path` ('/'-separated bytes) in `tree`, looked up name by name: no case
/// folding, and names that aren't UTF-8 work on every platform.
pub(super) fn tree_entry(repo: &Repository, tree: &Tree<'_>, path: &[u8]) -> Option<TreeEntry<'static>> {
    let mut parts = path.split(|&b| b == b'/');
    let mut entry = tree.get_name_bytes(parts.next()?)?.to_owned();
    for name in parts {
        let sub = entry.to_object(repo).ok()?.peel_to_tree().ok()?;
        entry = sub.get_name_bytes(name)?.to_owned();
    }
    Some(entry)
}

/// An index entry for `path` with `entry`'s blob and mode and no cached stat data, so the
/// working file is always compared by content.
pub(super) fn entry_from(entry: &TreeEntry<'_>, path: &[u8]) -> Result<IndexEntry> {
    let shown = String::from_utf8_lossy(path);
    let mode = u32::try_from(entry.filemode()).map_err(|_| Error::Other(format!("{shown} has an unknown mode in HEAD")))?;
    let zero = IndexTime::new(0, 0);
    Ok(IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: entry.id(),
        flags: 0,
        flags_extended: 0,
        path: path.to_vec(),
    })
}

/// Puts `path` in `index` back to how `head` has it, as `git reset -- <path>` does for one
/// literal path: HEAD's blob when HEAD has a file there, otherwise no entry. Conflict stages
/// for the path go too. `index` comes from `index` above.
pub(super) fn reset_path(repo: &Repository, index: &mut Index, head: Option<&Tree<'_>>, path: &str) -> Result<()> {
    for stage in 0..=3 {
        if index.get_path(Path::new(path), stage).is_some() {
            index.remove(Path::new(path), stage)?;
        }
    }
    let file = head.and_then(|t| tree_entry(repo, t, path.as_bytes())).filter(|e| e.kind() != Some(ObjectType::Tree));
    if let Some(e) = file {
        index.add(&entry_from(&e, path.as_bytes())?)?;
    }
    Ok(())
}

/// Removes `path`'s entries from the index, matched exactly (no globs, no case folding).
pub(super) fn remove_path(repo: &Repository, path: &str) -> Result<()> {
    let mut idx = index(repo)?;
    reset_path(repo, &mut idx, None, path)?;
    idx.write()?;
    Ok(())
}

/// Applies `diff`'s changes to `path` to the index (`opts` skips the deltas of any other path), reading the old
/// contents from `path`'s own stage-0 entry. `repo.apply` to the index looks the path up in the
/// repository's index, which folds case with `core.ignorecase`, so it could patch a case twin's
/// contents and store them under either name. Here the patch is applied to a tree that holds
/// only `path`, and the result is copied into the exactly-matching index.
pub(super) fn apply_to_index(repo: &Repository, diff: &Diff<'_>, path: &str, opts: &mut ApplyOptions<'_>) -> Result<()> {
    let mut index = index(repo)?;
    let before = index.get_path(Path::new(path), 0);
    let base = repo.find_tree(tree_with(repo, path, before.as_ref())?)?;
    let after = repo.apply_to_tree(&base, diff, Some(opts))?;
    match after.get_path(Path::new(path), 0) {
        Some(mut e) => {
            // Compared by content next time: the patched blob was never on disk.
            e.ctime = IndexTime::new(0, 0);
            e.mtime = IndexTime::new(0, 0);
            e.file_size = 0;
            index.add(&e)?;
        }
        None if before.is_some() => index.remove(Path::new(path), 0)?,
        None => {}
    }
    index.write()?;
    Ok(())
}

/// A tree holding only `entry`'s blob at `path` (nested folders made as needed), or an empty
/// tree without an entry.
fn tree_with(repo: &Repository, path: &str, entry: Option<&IndexEntry>) -> Result<Oid> {
    let Some(e) = entry else { return Ok(repo.treebuilder(None)?.write()?) };
    let (mut id, mut mode) = (e.id, i32::try_from(e.mode).map_err(|_| Error::Other(format!("{path} has an unknown mode")))?);
    for name in path.rsplit('/') {
        let mut b = repo.treebuilder(None)?;
        b.insert(name, id, mode)?;
        id = b.write()?;
        mode = 0o040000;
    }
    Ok(id)
}
