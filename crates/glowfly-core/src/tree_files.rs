//! The work tree's `.gitattributes` and `.gitignore`, read the way git reads them, as
//! `gitmodules.rs` does for `.gitmodules`: only a regular file, never through a link (git takes
//! a linked one as absent), and never more than [`MAX_BYTES`].
//!
//! libgit2 reads both files by itself for a status, a diff to the working tree and an attribute
//! or ignore lookup, and opens whatever a link there names. So before those reads the work tree
//! is checked ([`check`]): where one of the two is a link leading to a place
//! `net_path::may_follow` refuses from the work tree, the read is not made and the caller gets
//! `Error::LinkedFile` naming the file (docs/PLATFORMS.md, "Network places and links").

use crate::config_includes::read_file_bounded;
use crate::error::{Error, Result};
use git2::Repository;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The two files, as git names them.
const NAMES: [&str; 2] = [".gitattributes", ".gitignore"];

/// The largest of these files read here.
pub(crate) const MAX_BYTES: u64 = 1 << 20;

/// A symlink's mode in the index.
const LINK: u32 = 0o120000;

/// `file`'s bytes when it is a regular file (not a link to one) of at most [`MAX_BYTES`]; None
/// for anything else, as for no file. One stat and one bounded read.
pub(crate) fn read(file: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::symlink_metadata(file).ok()?;
    if !meta.file_type().is_file() || meta.len() > MAX_BYTES {
        return None;
    }
    read_file_bounded(file, MAX_BYTES)
}

/// Ok unless `linked_away` names a file: the check before libgit2 reads `repo`'s work tree.
pub(crate) fn check(repo: &Repository) -> Result<()> {
    match linked_away(repo) {
        Some(file) => Err(Error::LinkedFile(file)),
        None => Ok(()),
    }
}

/// The first `.gitattributes` or `.gitignore` in `repo`'s work tree (as shown, relative to it)
/// that is a link leading where `net_path::may_follow` refuses from the work tree. Looked at:
/// the two at the root, tracked or not, and those in each folder where the index holds a link
/// a volume could take for one of them (`may_name`). The folder is asked for the two names, so
/// the volume itself decides which entry answers to them. None in a bare repository. Per
/// folder looked at: two stats, and a link's own walk.
pub(crate) fn linked_away(repo: &Repository) -> Option<String> {
    let workdir = repo.workdir()?;
    let mut folders: BTreeSet<Vec<u8>> = BTreeSet::from([Vec::new()]);
    if let Ok(mut index) = repo.index() {
        let _ = index.read(false);
        for e in index.iter().filter(|e| e.mode == LINK) {
            let (dir, name) = match e.path.iter().rposition(|b| *b == b'/') {
                Some(at) => (&e.path[..at], &e.path[at + 1..]),
                None => (&e.path[..0], &e.path[..]),
            };
            if may_name(name) && !folders.contains(dir) {
                folders.insert(dir.to_vec());
            }
        }
    }
    for dir in &folders {
        let Some(folder) = folder_of(workdir, dir) else { continue };
        for name in NAMES {
            let file = folder.join(name);
            let linked = std::fs::symlink_metadata(&file).is_ok_and(|m| m.file_type().is_symlink());
            if linked && !crate::net_path::may_follow(&file, workdir) {
                let shown = crate::repo::shown_path(dir);
                return Some(if shown.is_empty() { name.to_owned() } else { format!("{shown}/{name}") });
            }
        }
    }
    None
}

/// Whether some volume could take the index name `name` (one path part) for `.gitattributes` or
/// `.gitignore`. Wider than any volume's rules, since the folder is asked afterwards: the name
/// in any letter case with anything after it (trailing dots and spaces, a stream name), a short
/// 8.3 name (`~`), and any name with a byte outside ASCII (characters a volume ignores or folds).
fn may_name(name: &[u8]) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.is_ascii() || name.contains(&b'~') || NAMES.iter().any(|n| lower.starts_with(n.as_bytes()))
}

/// The folder the index names `dir` ('/'-separated bytes; empty for the root) in `workdir`.
/// None for a name that can't be a folder inside the work tree here.
fn folder_of(workdir: &Path, dir: &[u8]) -> Option<PathBuf> {
    match std::str::from_utf8(dir) {
        Ok("") => Some(workdir.to_path_buf()),
        Ok(text) => crate::repo::work_path(workdir, text).ok(),
        #[cfg(unix)]
        Err(_) => {
            use std::os::unix::ffi::OsStrExt;
            Some(dir.split(|b| *b == b'/').fold(workdir.to_path_buf(), |at, part| at.join(std::ffi::OsStr::from_bytes(part))))
        }
        #[cfg(not(unix))]
        Err(_) => None,
    }
}

#[cfg(test)]
#[path = "tree_files_tests.rs"]
mod tests;
