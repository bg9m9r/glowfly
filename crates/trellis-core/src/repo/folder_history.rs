//! Folder history: the commits that changed anything under one folder, newest first, each with
//! the files it changed there. The walk is the file history's (`history::walk`), with git's
//! default simplification (a commit whose folder equals one parent's is skipped); the folder's
//! own renames are not followed.

use super::FileEntry;
use super::history::{self, HistoryEntry, HistoryStart, Kind, Queue, entry_of, present};
use crate::error::{Error, Result};
use git2::{Commit, Oid, Repository, Tree};
use std::path::Path;

/// The commits that changed something under `folder` (repo-relative, '/'-separated, without a
/// trailing '/'), newest first, examining at most `limit` commits.
pub fn folder_history(workdir: &Path, folder: &str, start: &HistoryStart, limit: usize) -> Result<history::FileHistory> {
    if folder.is_empty() {
        return Err(Error::Unsupported("A folder history needs a folder".into()));
    }
    history::walk(workdir, folder, start, limit, Kind::Folder)
}

/// Looks at one commit: records an entry if it changed something under the folder `p`, and
/// queues the parents to follow.
pub(super) fn examine(repo: &Repository, oid: Oid, p: &str, queue: &mut Queue, entries: &mut Vec<HistoryEntry>) -> Result<()> {
    let c = repo.find_commit(oid)?;
    let tree = c.tree()?;
    let me = present(&tree, p, Kind::Folder);
    let parents: Vec<Commit<'_>> = c.parents().collect();
    let trees: Vec<Tree<'_>> = parents.iter().map(|q| q.tree()).collect::<std::result::Result<_, _>>()?;
    let theirs: Vec<Option<(Oid, i32)>> = trees.iter().map(|t| present(t, p, Kind::Folder)).collect();

    if parents.is_empty() {
        if me.is_some() {
            entries.push(entry_of(&c, p, None, 'A', files_under(repo, None, &tree, p)?));
        }
        return Ok(());
    }
    if me.is_some()
        && let Some(i) = theirs.iter().position(|t| *t == me)
    {
        // Unchanged against this parent: the folder's history is that parent's.
        queue.push(&parents[i], p);
        return Ok(());
    }
    let queue_holders = |queue: &mut Queue| {
        for (q, t) in parents.iter().zip(&theirs) {
            if t.is_some() {
                queue.push(q, p);
            }
        }
    };
    if me.is_none() {
        // Only a start commit can lack the folder: it removed the folder, or never had it.
        if let Some(i) = theirs.iter().position(Option::is_some) {
            entries.push(entry_of(&c, p, None, 'D', files_under(repo, Some(&trees[i]), &tree, p)?));
            queue_holders(queue);
        }
        return Ok(());
    }
    // Changed against every parent.
    queue_holders(queue);
    let status = if theirs.iter().all(Option::is_none) { 'A' } else { 'M' };
    entries.push(entry_of(&c, p, None, status, files_under(repo, Some(&trees[0]), &tree, p)?));
    Ok(())
}

/// The files under `folder` that differ from `parent` (none: an empty tree) to `tree`, by their
/// full paths. A file moved in or out of the folder is 'A' or 'D'; a rename within it is 'R'.
fn files_under(repo: &Repository, parent: Option<&Tree<'_>>, tree: &Tree<'_>, folder: &str) -> Result<Vec<FileEntry>> {
    let mut opts = super::diff::path_opts(folder);
    let mut diff = repo.diff_tree_to_tree(parent, Some(tree), Some(&mut opts))?;
    diff.find_similar(None)?;
    let under = |b: Option<&[u8]>| b.is_some_and(|b| super::rpath::under(b, folder));
    Ok(diff
        .deltas()
        .filter(|d| under(d.new_file().path_bytes()) || under(d.old_file().path_bytes()))
        .map(|d| super::rpath::entry(super::rpath::delta_bytes(&d), super::diff::delta_char(d.status())))
        .collect())
}
