//! The files a stash changes, for the question before applying it on top of uncommitted
//! changes: what its working-tree commit, its index commit and its untracked-files commit (from
//! `git stash push -u`) change against the commit it was made on.

use crate::error::Result;
use git2::{Diff, Oid, Repository};
use std::collections::BTreeSet;
use std::path::Path;

/// The repository-relative, '/'-separated paths stash `oid` changes, sorted, each once (both
/// sides of a delta, so a delete and an add both count).
pub fn changed_paths(workdir: &Path, oid: Oid) -> Result<Vec<String>> {
    let repo = Repository::open(workdir)?;
    let stash = repo.find_commit(oid)?;
    let base = stash.parent(0)?.tree()?;
    let mut paths = BTreeSet::new();
    let mut add = |diff: Diff<'_>| {
        for d in diff.deltas() {
            for f in [d.old_file(), d.new_file()] {
                if let Some(p) = f.path_bytes() {
                    paths.insert(String::from_utf8_lossy(p).into_owned());
                }
            }
        }
    };
    add(repo.diff_tree_to_tree(Some(&base), Some(&stash.tree()?), None)?);
    if let Ok(index) = stash.parent(1) {
        add(repo.diff_tree_to_tree(Some(&base), Some(&index.tree()?), None)?);
    }
    // The untracked files are a root commit of their own: every file in it is one.
    if let Ok(untracked) = stash.parent(2) {
        add(repo.diff_tree_to_tree(None, Some(&untracked.tree()?), None)?);
    }
    Ok(paths.into_iter().collect())
}
