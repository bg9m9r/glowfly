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
//!
//! The network places the user agreed a folder's Git data may lead to (`Approved`, per folder)
//! are followed while that folder alone is looked at (`net_path::allowing`). A refusal names
//! every other network place found without following one; a place reached only through one
//! that isn't followed is found once that one is approved.

use super::{Found, Named, RepoDirs, common_dir_of, dirs_of, refusals, worktree_folder};
use crate::net_path::{Leads, allowing, leads_to, may_follow, place_text, root_of};
use crate::safe_directory::network_places::Approved;
use std::path::{Path, PathBuf};

/// Why a folder isn't opened.
#[derive(Debug, PartialEq)]
pub(crate) enum Refused {
    /// `folder`'s Git data leads to these network places, none of them approved for it (as
    /// `net_path::place_text` writes them, in the order found, each once).
    Network { folder: PathBuf, places: Vec<String> },
    /// Where this place, or the file naming it, leads can't be told (a file libgit2 would read
    /// differently, a name the system won't look up, too many object stores): never approved.
    Unchecked(PathBuf),
}

/// Whether libgit2 can look for the repository at `path` (and above it when `search`) without
/// following its files to a place that may not be followed from the folder holding them, or
/// one `approved` lists for that folder.
pub(crate) fn check(path: &Path, search: bool, approved: &Approved) -> Result<(), Refused> {
    // A path that can't be made absolute fails libgit2's own open.
    let Ok(spelled) = std::path::absolute(path) else { return Ok(()) };
    let start = {
        let _allowed = allowing(approved.places_for(&spelled));
        start_of(&spelled)?
    };
    for folder in start.ancestors() {
        let _allowed = allowing(approved.places_for(folder));
        if look(folder).map_err(|found| refused(folder, folder, found))? || !search {
            break;
        }
    }
    Ok(())
}

/// What the places `found` in `folder`'s Git data are, told from `home` (the place in use)
/// while the folder's approved places are allowed: unchecked when any one can't be told, else
/// the network places they lead to.
fn refused(folder: &Path, home: &Path, found: Vec<PathBuf>) -> Refused {
    let mut places: Vec<String> = Vec::new();
    for p in found {
        let text = match leads_to(&p, home) {
            Leads::Network(root) => place_text(&root),
            _ => None,
        };
        match text {
            Some(text) if !places.contains(&text) => places.push(text),
            Some(_) => {}
            None => return Refused::Unchecked(p),
        }
    }
    Refused::Network { folder: folder.to_path_buf(), places }
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
    let anchor = anchor.unwrap_or(spelled);
    if may_follow(spelled, anchor) {
        Ok(std::fs::canonicalize(spelled).unwrap_or_else(|_| spelled.to_path_buf()))
    } else {
        Err(refused(spelled, anchor, vec![spelled.to_path_buf()]))
    }
}

/// Whether libgit2 finds a repository in `folder`: its `.git`, else the folder itself as a git
/// dir. Err with the places found that may not be followed.
fn look(folder: &Path) -> Result<bool, Vec<PathBuf>> {
    match dirs_of(folder, folder) {
        Found::Refused(p) => return Err(vec![p]),
        Found::Repo(d) if taken(&d, folder)? => return Ok(true),
        _ => {}
    }
    // libgit2 reads a `commondir` in the folder only when it tries the folder as a git dir: a
    // file of that name (it asks for a file, so a folder named so is passed over).
    let named = std::fs::symlink_metadata(folder.join("commondir")).is_ok_and(|m| !m.is_dir());
    let common = match if named { common_dir_of(folder, folder) } else { Named::None } {
        Named::Refused(p) => return Err(vec![p]),
        Named::At(common) => common,
        Named::None => folder.to_path_buf(),
    };
    taken(&RepoDirs { git_dir: folder.to_path_buf(), common }, folder)
}

/// Whether libgit2 takes `d` (found in `folder`) for a repository, as its
/// `is_valid_repository_path` tells; then everything opening it follows is checked. Err with
/// every place found that may not be followed (`git_file::refusals`).
fn taken(d: &RepoDirs, folder: &Path) -> Result<bool, Vec<PathBuf>> {
    for (file, is_dir) in [(d.git_dir.join("HEAD"), false), (d.common.join("objects"), true), (d.common.join("refs"), true)] {
        // libgit2 stats each through a link.
        if !may_follow(&file, folder) {
            return Err(vec![file]);
        }
        if !std::fs::metadata(&file).is_ok_and(|m| if is_dir { m.is_dir() } else { m.is_file() }) {
            return Ok(false);
        }
    }
    let mut found = Vec::new();
    // A linked worktree's work tree is the folder its `gitdir` record names.
    if d.common != d.git_dir
        && let Named::Refused(p) = worktree_folder(&d.git_dir, folder)
    {
        found.push(p);
    }
    found.extend(refusals(d, folder, folder, true));
    if found.is_empty() { Ok(true) } else { Err(found) }
}

#[cfg(test)]
#[path = "opened_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "opened_approved_tests.rs"]
mod approved_tests;
