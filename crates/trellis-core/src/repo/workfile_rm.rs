//! Deleting a new file that is staged (or the new path of a staged rename): the working copy and
//! its index entries together, and taking one whose working copy is already gone out of the index.
//!
//! The index entries are changed with libgit2 rather than the git CLI: no hooks, config or
//! credentials apply to taking a path out of the index.

use super::head_tree;
use super::workfile::contained;
use crate::error::{Error, Result};
use git2::{Delta, DiffFindOptions, IndexEntry, IndexTime, Repository};
use std::path::Path;

/// Deletes the working-tree file `path`, which is staged as a new file or as the new path of a
/// staged rename, and removes its index entry, so the path leaves both the staged and unstaged
/// lists. For a rename, the source's index entry is put back from HEAD, so the source's
/// deletion is no longer staged: it stays an unstaged deletion, which discarding undoes. Empty
/// parent folders are pruned as `delete_file` does.
///
/// Refused, changing nothing, when `path` has no stage-0 index entry (untracked, or
/// conflicted) or is in HEAD's tree (then it isn't new and only its working copy should go),
/// and for every reason `delete_file` refuses.
///
/// The working copy goes first: when that fails nothing has changed, and when changing the
/// index afterwards fails the result is what a plain delete leaves (a staged new file deleted
/// from the working tree), which discarding the unstaged deletion brings back.
pub fn delete_staged_new_file(workdir: &Path, path: &str) -> Result<()> {
    let source = {
        let repo = Repository::open(workdir)?;
        check_staged_new(&repo, path)?
        // The repository is dropped here: Windows locks files that are held open.
    };
    super::workfile::delete_file(workdir, path)?;
    unstage(workdir, path, source.as_deref()).map_err(|e| Error::Other(format!("{path} was deleted but is still staged: {e}")))
}

/// Takes `path`, staged as a new file or as the new path of a staged rename but already gone
/// from the working tree, out of the index (a rename's source gets its HEAD entry back, as in
/// `delete_staged_new_file`). Nothing on disk is created or removed.
///
/// Refused, changing nothing, for the reasons `delete_staged_new_file` gives, and when
/// something other than a folder is at `path` again: deleting that wasn't agreed to.
pub fn unstage_gone_new_file(workdir: &Path, path: &str) -> Result<()> {
    let path = contained(path)?;
    let source = {
        let repo = Repository::open(workdir)?;
        let root = repo.workdir().ok_or_else(|| Error::Unsupported("a bare repository has no working tree".into()))?;
        let source = check_staged_new(&repo, path)?;
        // A folder there now doesn't matter: nothing on disk is removed.
        if root.join(path).symlink_metadata().is_ok_and(|m| !m.is_dir()) {
            return Err(Error::Other(format!("{path} is in your working tree again; refresh and delete it from there")));
        }
        source
    };
    unstage(workdir, path, source.as_deref())
}

/// Checks that `path` is staged as new (a stage-0 entry that HEAD's tree lacks) and returns the
/// source of the staged rename it is the new path of, if any. Run before anything changes:
/// removing the new path's entry dissolves the rename.
fn check_staged_new(repo: &Repository, path: &str) -> Result<Option<String>> {
    if repo.index()?.get_path(Path::new(path), 0).is_none() {
        return Err(Error::Other(format!("{path} isn't staged")));
    }
    // An unborn HEAD has no tree, so every staged path is new.
    if head_tree(repo).is_some_and(|t| t.get_path(Path::new(path)).is_ok()) {
        return Err(Error::Other(format!("{path} is in the last commit, so it isn't a new file")));
    }
    rename_source(repo, path)
}

/// The old path of the staged rename whose new path is `path`, as status pairs it (HEAD against
/// the index). Explicit options, because the default reads `diff.renames` from config.
fn rename_source(repo: &Repository, path: &str) -> Result<Option<String>> {
    let Some(tree) = head_tree(repo) else { return Ok(None) };
    let index = repo.index()?;
    let mut diff = repo.diff_tree_to_index(Some(&tree), Some(&index), None)?;
    diff.find_similar(Some(DiffFindOptions::new().renames(true)))?;
    let source = diff
        .deltas()
        .find(|d| d.status() == Delta::Renamed && d.new_file().path() == Some(Path::new(path)))
        .and_then(|d| d.old_file().path().map(|p| p.to_string_lossy().into_owned()));
    Ok(source)
}

/// Takes `path` out of the index and gives `source` back its HEAD entry, in one index write.
///
/// Both are exact index edits rather than `unstage_paths`: that reads its paths as pathspecs, so
/// a name such as `[id].tsx` would also reset every path it matches as a glob (`i.tsx`,
/// `d.tsx`) and lose their staged changes.
fn unstage(workdir: &Path, path: &str, source: Option<&str>) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = repo.index()?;
    index.remove(Path::new(path), 0)?;
    if let Some(source) = source {
        // A rename source is in HEAD's tree: that is how status paired it.
        let tree = head_tree(&repo).ok_or_else(|| Error::Other("HEAD has no tree".into()))?;
        let entry = tree.get_path(Path::new(source))?;
        // With `core.ignorecase`, a case-only rename's source and new path are one index path;
        // the new path's entry is already gone, so adding the source can't replace anything else.
        let zero = IndexTime::new(0, 0);
        index.add(&IndexEntry {
            // No cached stat data, so the working file is always compared by content.
            ctime: zero,
            mtime: zero,
            dev: 0,
            ino: 0,
            mode: u32::try_from(entry.filemode()).map_err(|_| Error::Other(format!("{source} has an unknown mode in HEAD")))?,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: entry.id(),
            flags: 0,
            flags_extended: 0,
            path: source.as_bytes().to_vec(),
        })?;
    }
    index.write()?;
    Ok(())
}
