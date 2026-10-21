//! Folder history: the commits that changed anything under one folder, newest first, each with
//! the files it changed there. The walk is the file history's (`history::walk`), with git's
//! default simplification (a commit whose folder equals one parent's is skipped); the folder's
//! own renames are followed backwards (`moved_from`).

use super::FileEntry;
use super::history::{self, HistoryEntry, HistoryStart, Kind, Queue, entry_of, present};
use crate::error::{Error, Result};
use git2::{Commit, Delta, DiffFindOptions, Oid, Repository, Tree};
use std::collections::BTreeMap;
use std::path::Path;

/// How many files a whole-tree rename search may compare, so a big folder's move is found.
const FOLDER_RENAME_LIMIT: usize = 5000;

/// A file of a folder entry by its name today: `file` (under `entry_folder`, the folder's name
/// in that commit) moved under `folder_now`; a file outside `entry_folder` keeps its path.
pub fn folder_file_today(entry_folder: &str, folder_now: &str, file: &str) -> String {
    match file.strip_prefix(entry_folder).and_then(|r| r.strip_prefix('/')) {
        Some(rest) => format!("{folder_now}/{rest}"),
        None => file.to_owned(),
    }
}

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
    // Changed against every parent. A parent without the folder may have had it under another
    // name; the first parent decides whether this commit is the move.
    let mut first_moved = None;
    let mut any_moved = false;
    for (i, ((q, qt), t)) in parents.iter().zip(&trees).zip(&theirs).enumerate() {
        if t.is_some() {
            queue.push(q, p);
            continue;
        }
        if let Some(old) = moved_from(repo, qt, &tree, p)? {
            queue.push(q, &old);
            any_moved = true;
            if i == 0 {
                first_moved = Some(old);
            }
        }
    }
    if let Some(old) = first_moved {
        let files = files_moved(repo, &trees[0], &tree, p, &old)?;
        entries.push(entry_of(&c, p, Some(old), 'R', files));
        return Ok(());
    }
    let status = if !any_moved && theirs.iter().all(Option::is_none) { 'A' } else { 'M' };
    entries.push(entry_of(&c, p, None, status, files_under(repo, Some(&trees[0]), &tree, p)?));
    Ok(())
}

/// The folder that `folder` (in `tree`, missing from `parent_tree`) was moved from: the one
/// most of its files were renamed from under the same relative paths (ties: the first by
/// name), when it is a folder in `parent_tree`.
fn moved_from(repo: &Repository, parent_tree: &Tree<'_>, tree: &Tree<'_>, folder: &str) -> Result<Option<String>> {
    let mut diff = repo.diff_tree_to_tree(Some(parent_tree), Some(tree), None)?;
    diff.find_similar(Some(DiffFindOptions::new().renames(true).rename_limit(FOLDER_RENAME_LIMIT)))?;
    let (mut total, mut votes) = (0usize, BTreeMap::<String, usize>::new());
    for d in diff.deltas() {
        let Some(new) = d.new_file().path_bytes().and_then(super::rpath::text) else { continue };
        let Some(rest) = new.strip_prefix(folder).and_then(|r| r.strip_prefix('/')) else { continue };
        total += 1;
        if d.status() != Delta::Renamed {
            continue;
        }
        let Some(old) = d.old_file().path_bytes().and_then(super::rpath::text) else { continue };
        if let Some(src) = old.strip_suffix(rest).and_then(|s| s.strip_suffix('/'))
            && !src.is_empty()
            && src != folder
        {
            *votes.entry(src.to_owned()).or_default() += 1;
        }
    }
    // Strictly more votes replaces the best, so a tie keeps the first name.
    let best = votes.into_iter().fold(None, |best: Option<(String, usize)>, (src, n)| match best {
        Some((_, m)) if m >= n => best,
        _ => Some((src, n)),
    });
    Ok(best.filter(|(src, n)| n * 2 > total && present(parent_tree, src, Kind::Folder).is_some()).map(|(src, _)| src))
}

/// The files of a commit that moved `old` (in `parent`) to `folder` (in `tree`): those under
/// either folder that differ, a moved file as 'R' under its new path.
fn files_moved(repo: &Repository, parent: &Tree<'_>, tree: &Tree<'_>, folder: &str, old: &str) -> Result<Vec<FileEntry>> {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(folder).pathspec(old).disable_pathspec_match(true);
    let mut diff = repo.diff_tree_to_tree(Some(parent), Some(tree), Some(&mut opts))?;
    diff.find_similar(Some(DiffFindOptions::new().renames(true).rename_limit(FOLDER_RENAME_LIMIT)))?;
    let under = |b: Option<&[u8]>| b.is_some_and(|b| super::rpath::under(b, folder) || super::rpath::under(b, old));
    Ok(diff
        .deltas()
        .filter(|d| under(d.new_file().path_bytes()) || under(d.old_file().path_bytes()))
        .map(|d| super::rpath::entry(super::rpath::delta_bytes(&d), super::diff::delta_char(d.status())))
        .collect())
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
