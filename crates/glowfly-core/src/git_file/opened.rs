//! The folder the user opens, given the checks a submodule or nested repository gets before
//! libgit2 opens it (`other_repo`): its `.git` file, `commondir`, worktree record, the links in
//! its git dir, the files its config includes and the object stores its alternates name, each
//! followed only where `net_path::may_follow` allows from the folder itself. So a repository
//! that lives on a share opens as before (the share is the place in use), and one whose files
//! name another network place is not opened.
//!
//! The folders are looked at in the order libgit2's own search does (`find_repo`): for the
//! folder and then each one above it, first its `.git`, then the folder itself as a git dir,
//! stopping at the first that libgit2 would take for a repository (a `HEAD` file, with
//! `objects/` and `refs/` in its common dir). libgit2 reads a `commondir` file in every folder
//! it tries, so each is checked before the next is looked at.

use super::{Found, Named, RepoDirs, common_dir_of, dirs_of, openable, worktree_folder};
use crate::net_path::may_follow;
use std::path::{Path, PathBuf};

/// Whether libgit2 can look for the repository at `path` (and above it when `search`) without
/// following its files to a place that may not be followed from the folder holding them: Err
/// with the first such place.
pub(crate) fn check(path: &Path, search: bool) -> Result<(), PathBuf> {
    // A path that can't be made absolute fails libgit2's own open.
    let Ok(start) = std::path::absolute(path) else { return Ok(()) };
    for folder in start.ancestors() {
        let own = match dirs_of(folder, folder) {
            Found::Refused(p) => return Err(p),
            Found::Repo(d) => Some(d),
            Found::None => None,
        };
        let bare = match common_dir_of(folder, folder) {
            Named::Refused(p) => return Err(p),
            Named::At(common) => RepoDirs { git_dir: folder.to_path_buf(), common },
            Named::None => RepoDirs { git_dir: folder.to_path_buf(), common: folder.to_path_buf() },
        };
        for d in own.into_iter().chain([bare]) {
            if taken(&d, folder)? {
                return Ok(());
            }
        }
        if !search {
            break;
        }
    }
    Ok(())
}

/// Whether libgit2 takes `d` (found in `folder`) for a repository, as its
/// `is_valid_repository_path` tells; then everything opening it follows is checked. Err with
/// the first place that may not be followed.
fn taken(d: &RepoDirs, folder: &Path) -> Result<bool, PathBuf> {
    for (file, is_dir) in [(d.git_dir.join("HEAD"), false), (d.common.join("objects"), true), (d.common.join("refs"), true)] {
        // libgit2 stats each through a link.
        if !may_follow(&file, folder) {
            return Err(file);
        }
        if !std::fs::metadata(&file).is_ok_and(|m| if is_dir { m.is_dir() } else { m.is_file() }) {
            return Ok(false);
        }
    }
    // A linked worktree's work tree is the folder its `gitdir` record names.
    if d.common != d.git_dir
        && let Named::Refused(p) = worktree_folder(&d.git_dir, folder)
    {
        return Err(p);
    }
    openable(d, folder, folder)?;
    Ok(true)
}

#[cfg(test)]
#[path = "opened_tests.rs"]
mod tests;
