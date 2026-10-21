//! Per-path settings read from gitattributes: the working tree's and the index's, or a
//! commit's own `.gitattributes` files for commit and comparison diffs.

use super::DiffTarget;
use crate::error::{Error, Result};
use git2::{AttrCheckFlags, AttrValue, Index, IndexEntry, IndexTime, ObjectType, Repository, Tree};
use std::path::Path;

/// The `linguist-language` value for `path` (repo-relative, '/'-separated), from the working
/// tree's attributes then the index's (also `.git/info/attributes` and `core.attributesFile`).
/// `None` when unset, set without a value, unspecified, empty, not UTF-8, or on any libgit2 error.
pub(crate) fn linguist_language(repo: &Repository, path: &str) -> Option<String> {
    string_attr(repo, path, "linguist-language")
}

/// The `linguist-language` value for `path` in the diff `target`: from the tree of the commit
/// it shows (`attr_tree`), else as `linguist_language`.
pub(crate) fn linguist_language_for(repo: &Repository, target: &DiffTarget, path: &str) -> Option<String> {
    match attr_tree(repo, target) {
        Some(tree) => linguist_language_in(repo, &tree, path),
        None => linguist_language(repo, path),
    }
}

/// The tree whose `.gitattributes` files apply to `target`: the commit's for a commit diff, the
/// newer commit's for a comparison (the commit end when the other is the empty tree). `None`
/// for diffs against the index or the working tree.
pub(crate) fn attr_tree<'r>(repo: &'r Repository, target: &DiffTarget) -> Option<Tree<'r>> {
    let oid = match target {
        DiffTarget::Commit(oid, _) => *oid,
        DiffTarget::Between(a, b, _) => {
            if super::is_empty_tree(*b) {
                *a
            } else {
                *b
            }
        }
        _ => return None,
    };
    if super::is_empty_tree(oid) {
        return None;
    }
    repo.find_object(oid, None).ok()?.peel_to_tree().ok()
}

/// The `linguist-language` value for `path` from `tree`'s `.gitattributes` files (the root's and
/// each folder's on the way to `path`), plus `.git/info/attributes`, `core.attributesFile` and
/// the system file, as `git check-attr --source <tree>` reads them. They are read from an
/// in-memory index on a separate handle, so nothing reaches `repo` or its attribute cache.
/// `None` as for `linguist_language`, and on any error.
pub(crate) fn linguist_language_in(repo: &Repository, tree: &Tree, path: &str) -> Option<String> {
    let handle = Repository::open(repo.path()).ok()?;
    let mut index = Index::new().ok()?;
    let mut dir = String::new();
    let folders = path.rsplit_once('/').map_or("", |(d, _)| d);
    for part in std::iter::once("").chain(folders.split('/').filter(|p| !p.is_empty())) {
        if !part.is_empty() {
            dir = if dir.is_empty() { part.to_owned() } else { format!("{dir}/{part}") };
        }
        let file = if dir.is_empty() { ".gitattributes".to_owned() } else { format!("{dir}/.gitattributes") };
        let Ok(entry) = tree.get_path(Path::new(&file)) else { continue };
        if entry.kind() != Some(ObjectType::Blob) {
            continue;
        }
        let size = repo.find_blob(entry.id()).ok()?.size();
        index.add(&index_entry(file, entry.id(), size)).ok()?;
    }
    handle.set_index(&mut index).ok()?;
    let raw = handle.get_attr_bytes(Path::new(path), "linguist-language", AttrCheckFlags::INDEX_ONLY).ok()?;
    parse_value(raw)
}

/// An index entry for a regular file `path` with blob `id`.
fn index_entry(path: String, id: git2::Oid, size: usize) -> IndexEntry {
    let zero = IndexTime::new(0, 0);
    IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: u32::try_from(size).unwrap_or(u32::MAX),
        id,
        flags: 0,
        flags_extended: 0,
        path: path.into_bytes(),
    }
}

/// Whether `path` has `filter=lfs` (the working tree's attributes, then the index's).
pub(super) fn lfs_filtered(repo: &Repository, path: &str) -> bool {
    string_attr(repo, path, "filter").as_deref() == Some("lfs")
}

fn string_attr(repo: &Repository, path: &str, name: &str) -> Option<String> {
    parse_value(repo.get_attr_bytes(Path::new(path), name, AttrCheckFlags::FILE_THEN_INDEX).ok()?)
}

/// A string attribute's value, trimmed; `None` unless it is a non-empty UTF-8 string.
fn parse_value(raw: Option<&[u8]>) -> Option<String> {
    match AttrValue::from_bytes(raw) {
        AttrValue::String(s) => Some(s.trim()).filter(|s| !s.is_empty()).map(str::to_owned),
        _ => None,
    }
}

/// The content conversion git applies to `path` between the working tree and the index that
/// libgit2 can't run: an external `filter` driver (Git LFS, git-crypt, ...) or a
/// `working-tree-encoding`. Shown as the attribute, e.g. `filter=lfs`.
pub(crate) fn content_conversion(repo: &Repository, path: &str) -> Option<String> {
    ["filter", "working-tree-encoding"].into_iter().find_map(|name| string_attr(repo, path, name).map(|v| format!("{name}={v}")))
}

/// Refuses a patch-based edit of `path` (by hunk or line) when git converts its content on the
/// way in or out: the patch would be built and applied to unconverted bytes, storing the wrong
/// content. Whole-file operations go through git and are fine.
pub(crate) fn refuse_converted(repo: &Repository, path: &str, what: &str) -> Result<()> {
    match content_conversion(repo, path) {
        Some(attr) => Err(Error::Unsupported(format!(
            "{path} has {attr} in .gitattributes, so git converts its content and {what} by hunk or line would store the wrong bytes; use the whole-file action instead"
        ))),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "attrs_tests.rs"]
mod tests;
