//! The folder the user opens, given the checks a submodule or nested repository gets before
//! libgit2 opens it (`other_repo`): its `.git` file, `commondir`, worktree record, the links in
//! its git dir, the files its config includes and the object stores its alternates name, each
//! followed only where `net_path::may_follow` allows from the folder itself. So a repository
//! that lives on a share opens as before (the share is the place in use), and one whose files
//! name another network place is not opened.
//!
//! The folders are looked at in the order libgit2's own search does (`find_repo`): for the
//! folder and then each one above it, first its `.git`, then the folder itself as a git dir
//! (only then is a `commondir` file in it read), stopping at the first that libgit2 would take
//! for a repository (a `HEAD` file, with `objects/` and `refs/` in its common dir). On Unix
//! libgit2 resolves the links in the path it starts from, so the folders above are those of
//! the resolved path; on Windows it goes up the path as spelled.

use super::{Found, Named, RepoDirs, common_dir_of, dirs_of, openable, worktree_folder};
use crate::net_path::{Leads, leads_to, may_follow, root_of};
use std::path::{Path, PathBuf};

/// Why a folder isn't opened.
#[derive(Debug, PartialEq)]
pub(crate) struct Refused {
    /// The first place, or the file naming it, that may not be followed.
    pub place: PathBuf,
    /// It is another network place; otherwise where it leads can't be told (a file libgit2
    /// would read differently, a name the system won't look up, too many object stores).
    pub network: bool,
}

/// Whether libgit2 can look for the repository at `path` (and above it when `search`) without
/// following its files to a place that may not be followed from the folder holding them.
pub(crate) fn check(path: &Path, search: bool) -> Result<(), Refused> {
    // A path that can't be made absolute fails libgit2's own open.
    let Ok(spelled) = std::path::absolute(path) else { return Ok(()) };
    let start = start_of(&spelled)?;
    for folder in start.ancestors() {
        let refused = |place: PathBuf| Refused { network: leads_to(&place, folder) == Leads::Network, place };
        if look(folder).map_err(refused)? || !search {
            break;
        }
    }
    Ok(())
}

/// The folder libgit2 starts from for `spelled` (absolute). On Unix it resolves the path
/// first, so that is done here, once the links in it are known to stay on the place the path
/// itself is spelled on; one that leads elsewhere is refused, as what libgit2 would find can't
/// be looked at. A path that doesn't resolve is used as spelled (libgit2's open fails).
fn start_of(spelled: &Path) -> Result<PathBuf, Refused> {
    if cfg!(windows) {
        return Ok(spelled.to_path_buf());
    }
    // The place in use: the network root of the path it spells once its `.` and `..` are
    // taken out (by the text alone), or this computer. The walk itself goes by the spelling
    // as given, so every folder it passes through on the way is looked at.
    let mut plain = PathBuf::new();
    for part in spelled.components() {
        match part {
            std::path::Component::ParentDir => {
                plain.pop();
            }
            std::path::Component::CurDir => {}
            part => plain.push(part),
        }
    }
    let anchor = plain.ancestors().take_while(|a| root_of(a).is_some()).last().or_else(|| plain.ancestors().last());
    match leads_to(spelled, anchor.unwrap_or(spelled)) {
        Leads::Near => Ok(std::fs::canonicalize(spelled).unwrap_or_else(|_| spelled.to_path_buf())),
        far => Err(Refused { place: spelled.to_path_buf(), network: far == Leads::Network }),
    }
}

/// Whether libgit2 finds a repository in `folder`: its `.git`, else the folder itself as a git
/// dir. Err with the first place that may not be followed.
fn look(folder: &Path) -> Result<bool, PathBuf> {
    match dirs_of(folder, folder) {
        Found::Refused(p) => return Err(p),
        Found::Repo(d) if taken(&d, folder)? => return Ok(true),
        _ => {}
    }
    // libgit2 reads a `commondir` in the folder only when it tries the folder as a git dir: a
    // file of that name (it asks for a file, so a folder named so is passed over).
    let named = std::fs::symlink_metadata(folder.join("commondir")).is_ok_and(|m| !m.is_dir());
    let common = match if named { common_dir_of(folder, folder) } else { Named::None } {
        Named::Refused(p) => return Err(p),
        Named::At(common) => common,
        Named::None => folder.to_path_buf(),
    };
    taken(&RepoDirs { git_dir: folder.to_path_buf(), common }, folder)
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
