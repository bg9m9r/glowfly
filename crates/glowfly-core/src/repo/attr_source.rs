//! Where gitattributes are read from for the version a view shows: the working tree's files
//! (then the index's), the index's alone (staged content), or a commit's own `.gitattributes`
//! files (a version in history). `AttrRepo` is a repository handle whose attribute lookups,
//! libgit2's own for diffs included (binary detection, diff drivers), read that source.
//!
//! libgit2 can read a commit's files itself (`GIT_ATTR_CHECK_INCLUDE_COMMIT`), but only through
//! `git_attr_options`, which git2 doesn't wrap, and only after the working tree's and the
//! index's files. So a commit's files are put in an in-memory index on a separate bare handle,
//! where the index is the only source: the root's `.gitattributes` and each folder's on the way
//! to the paths asked about, with `$GIT_DIR/info/attributes`, `core.attributesFile` and the
//! system file read as usual, as `git check-attr --source <commit>` reads them. Nothing reaches
//! the caller's handle or its attribute cache.

use super::DiffTarget;
use git2::{AttrCheckFlags, AttrValue, Diff, Index, IndexEntry, IndexTime, ObjectType, Oid, Repository, RepositoryOpenFlags, Tree};
use std::collections::BTreeSet;
use std::ops::Deref;
use std::path::Path;

/// Where a path's gitattributes are read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrSource {
    /// The working tree's files, then the index's: for working-tree content.
    Workdir,
    /// The index's files only: for staged content.
    Index,
    /// The `.gitattributes` files in this commit's tree: for a version in history.
    Commit(Oid),
}

impl AttrSource {
    /// The source for the new side of `target`'s diff: the commit for a commit diff, the newer
    /// commit for a comparison (the commit end when the other is the empty tree), the index for
    /// staged changes, the working tree for the rest.
    pub fn of_target(target: &DiffTarget) -> Self {
        let oid = match target {
            DiffTarget::Commit(oid, _) => *oid,
            DiffTarget::Between(a, b, _) => {
                if super::is_empty_tree(*b) {
                    *a
                } else {
                    *b
                }
            }
            DiffTarget::Staged(_) => return AttrSource::Index,
            DiffTarget::Unstaged(_) | DiffTarget::ToWorkdir(..) | DiffTarget::FromWorkdir(..) => return AttrSource::Workdir,
        };
        if super::is_empty_tree(oid) { AttrSource::Workdir } else { AttrSource::Commit(oid) }
    }
}

/// A handle whose attribute lookups read one source: the caller's own (the working tree, or the
/// index with `INDEX_ONLY`), or a separate bare one (the index for libgit2's own lookups in
/// diffs, or a commit's files in an in-memory index). Derefs to the handle.
pub(crate) struct AttrRepo<'r> {
    handle: Handle<'r>,
    flags: AttrCheckFlags,
}

enum Handle<'r> {
    Same(&'r Repository),
    Own(Repository),
}

impl Deref for AttrRepo<'_> {
    type Target = Repository;
    fn deref(&self) -> &Repository {
        match &self.handle {
            Handle::Same(r) => r,
            Handle::Own(r) => r,
        }
    }
}

impl AttrRepo<'_> {
    /// Attribute `name` for `path` (repo-relative, '/'-separated); unspecified on any error.
    pub(crate) fn attr(&self, path: &str, name: &str) -> AttrValue<'_> {
        self.get_attr_bytes(Path::new(path), name, self.flags).map_or(AttrValue::Unspecified, AttrValue::from_bytes)
    }
}

/// A handle reading `source`'s attributes for `paths` (repo-relative, '/'-separated; only a
/// commit's source needs them, to know which folders' files to load). Falls back to `repo`
/// itself, reading the working tree's, if the separate handle can't be made.
pub(crate) fn attr_repo<'r>(repo: &'r Repository, source: AttrSource, paths: &[&str]) -> AttrRepo<'r> {
    let own = match source {
        AttrSource::Workdir => None,
        AttrSource::Index => bare_handle(repo),
        AttrSource::Commit(oid) => bare_handle(repo).filter(|h| point_at(h, oid, paths)),
    };
    match own {
        Some(h) => AttrRepo { handle: Handle::Own(h), flags: AttrCheckFlags::INDEX_ONLY },
        None => AttrRepo { handle: Handle::Same(repo), flags: AttrCheckFlags::FILE_THEN_INDEX },
    }
}

/// `attr_repo` for the new side of `target`'s diff, for its own path.
pub(crate) fn for_target<'r>(repo: &'r Repository, target: &DiffTarget) -> AttrRepo<'r> {
    attr_repo(repo, AttrSource::of_target(target), &[target.path()])
}

/// The paths on both sides of `diff`'s deltas, for `attr_repo`.
pub(crate) fn delta_paths(diff: &Diff) -> Vec<String> {
    let mut out = BTreeSet::new();
    for d in diff.deltas() {
        for f in [d.old_file(), d.new_file()] {
            if let Some(p) = f.path_bytes().and_then(super::rpath::text) {
                out.insert(p);
            }
        }
    }
    out.into_iter().collect()
}

/// A bare handle on `repo`'s git folder: with no working tree, its lookups never read the
/// working tree's files, and libgit2's own (in diffs) read its index alone. `point_at` gives it
/// a commit's files instead of the index.
pub(crate) fn bare_handle(repo: &Repository) -> Option<Repository> {
    let flags = RepositoryOpenFlags::NO_SEARCH | RepositoryOpenFlags::BARE;
    Repository::open_ext(repo.path(), flags, std::iter::empty::<&std::ffi::OsStr>()).ok()
}

/// Gives `handle` an in-memory index of commit `oid`'s `.gitattributes` files for `paths` (see
/// the module doc). Can be called again to point the handle at another commit. False on error.
pub(crate) fn point_at(handle: &Repository, oid: Oid, paths: &[&str]) -> bool {
    let Some(tree) = handle.find_object(oid, None).ok().and_then(|o| o.peel_to_tree().ok()) else { return false };
    attr_index(&tree, paths).is_some_and(|mut index| handle.set_index(&mut index).is_ok())
}

fn attr_index(tree: &Tree, paths: &[&str]) -> Option<Index> {
    let mut index = Index::new().ok()?;
    for file in attr_files(paths) {
        let Ok(entry) = tree.get_path(Path::new(&file)) else { continue };
        if entry.kind() != Some(ObjectType::Blob) {
            continue;
        }
        index.add(&index_entry(file, entry.id())).ok()?;
    }
    Some(index)
}

/// The `.gitattributes` files that can apply to `paths`: the root's and each folder's on the
/// way to each path.
fn attr_files(paths: &[&str]) -> BTreeSet<String> {
    let mut out = BTreeSet::from([".gitattributes".to_owned()]);
    for path in paths {
        let folders = path.rsplit_once('/').map_or("", |(d, _)| d);
        let mut dir = String::new();
        for part in folders.split('/').filter(|p| !p.is_empty()) {
            dir = if dir.is_empty() { part.to_owned() } else { format!("{dir}/{part}") };
            out.insert(format!("{dir}/.gitattributes"));
        }
    }
    out
}

/// An index entry for a regular file `path` with blob `id` (no stat data: nothing compares it
/// with a file on disk).
fn index_entry(path: String, id: Oid) -> IndexEntry {
    let zero = IndexTime::new(0, 0);
    IndexEntry {
        ctime: zero,
        mtime: zero,
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: 0,
        flags_extended: 0,
        path: path.into_bytes(),
    }
}

#[cfg(test)]
#[path = "attr_source_tests.rs"]
mod tests;
