//! Deleting a new file that is staged: the working copy and its index entry together.
//!
//! The index entry is removed with libgit2 rather than the git CLI: no hooks, config or
//! credentials apply to taking a path out of the index (as with `unstage_paths`).

use super::head_tree;
use crate::error::{Error, Result};
use git2::Repository;
use std::path::Path;

/// Deletes the working-tree file `path`, which is staged as a new file, and removes its index
/// entry, so the path leaves both the staged and unstaged lists. Empty parent folders are pruned
/// as `delete_file` does.
///
/// Refused, changing nothing, when `path` has no stage-0 index entry (untracked, or
/// conflicted) or is in HEAD's tree (then it isn't new and only its working copy should go),
/// and for every reason `delete_file` refuses.
///
/// The working copy goes first: when that fails nothing has changed, and when removing the
/// index entry afterwards fails the result is what a plain delete leaves (a staged new file
/// deleted from the working tree), which discarding the unstaged deletion brings back.
pub fn delete_staged_new_file(workdir: &Path, path: &str) -> Result<()> {
    {
        let repo = Repository::open(workdir)?;
        let index = repo.index()?;
        if index.get_path(Path::new(path), 0).is_none() {
            return Err(Error::Other(format!("{path} isn't staged")));
        }
        // An unborn HEAD has no tree, so every staged path is new.
        if head_tree(&repo).is_some_and(|t| t.get_path(Path::new(path)).is_ok()) {
            return Err(Error::Other(format!("{path} is in the last commit, so it isn't a new file")));
        }
        // The index and repository are dropped here: Windows locks files that are held open.
    }
    super::workfile::delete_file(workdir, path)?;
    remove_entry(workdir, path).map_err(|e| Error::Other(format!("{path} was deleted but is still staged: {e}")))
}

fn remove_entry(workdir: &Path, path: &str) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = repo.index()?;
    index.remove_path(Path::new(path))?;
    index.write()?;
    Ok(())
}
