//! The work tree's `.gitattributes` and `.gitignore`, read the way git reads them, as
//! `gitmodules.rs` does for `.gitmodules`: only a regular file, never through a link (git takes
//! a linked one as absent), and never more than [`MAX_BYTES`].
//!
//! libgit2 reads both files by itself for a status, a diff to the working tree and an attribute
//! or ignore lookup, and opens whatever a link there names. So before those reads the work tree
//! is checked ([`check`]): where one of the two is a link leading to a place
//! `net_path::may_follow` refuses from the work tree, the read is not made and the caller gets
//! `Error::LinkedFile` naming the file (docs/PLATFORMS.md, "Network places and links"). A
//! checked-out submodule's two files are checked the same way ([`linked_away_at`]), and one that
//! fails counts among the submodules libgit2 is kept from (`git_file::submodules_unread`).

use crate::config_includes::read_file_bounded;
use crate::error::{Error, Result};
use git2::Repository;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The two files, as git names them.
const NAMES: [&str; 2] = [".gitattributes", ".gitignore"];

/// The largest of these files read here: room for the attributes of a large LFS repository.
pub(crate) const MAX_BYTES: u64 = 8 << 20;

/// The most index files whose link folders are remembered (`link_folders`).
const MAX_REMEMBERED: usize = 4096;

/// A symlink's mode in the index.
const LINK: u32 = 0o120000;

/// `file`'s bytes when it is a regular file (not a link to one) of at most [`MAX_BYTES`]; None
/// for anything else, as for no file. One stat and one bounded read.
pub(crate) fn read(file: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::symlink_metadata(file).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    let bytes = if meta.len() > MAX_BYTES { None } else { read_file_bounded(file, MAX_BYTES) };
    if bytes.is_none() {
        tracing::debug!(file = %file.display(), limit = MAX_BYTES, "not read: over the size limit, or it can't be read");
    }
    bytes
}

/// Ok unless `linked_away` names a file: the check before libgit2 reads `repo`'s work tree.
pub(crate) fn check(repo: &Repository) -> Result<()> {
    let _span = tracing::debug_span!("tree_files.check").entered();
    match linked_away(repo) {
        Some(file) => Err(Error::LinkedFile(file)),
        None => Ok(()),
    }
}

/// The first `.gitattributes` or `.gitignore` in `repo`'s work tree (as shown, relative to it)
/// that libgit2 must be kept from (`linked_away_at`). None in a bare repository.
pub(crate) fn linked_away(repo: &Repository) -> Option<String> {
    let workdir = repo.workdir()?;
    linked_away_at(workdir, workdir, &repo.path().join("index"))
}

/// The first `.gitattributes` or `.gitignore` in the work tree `workdir` (as shown, relative to
/// it) that `net_path::may_follow` refuses from `home`: it, or a folder on the way to it, is a
/// link leading there, or can't be looked at. Looked at: the two at the root, tracked or not, and
/// those in each folder where the index file `index` holds a link a volume could take for one of
/// them (`may_name`). The folder is asked for the two names, so the volume itself decides which
/// entry answers to them. `may_follow` looks at each part without following it, so nothing past
/// a refused link is touched. The root's two are asked in `NAMES` order; a tracked folder's with
/// the name its index entry spells first, so that is the file named when the folder itself
/// leads away. Per folder: the stats of the two walks.
pub(crate) fn linked_away_at(workdir: &Path, home: &Path, index: &Path) -> Option<String> {
    let tracked = link_folders(index).unwrap_or_default();
    let root: Tracked = (Vec::new(), None);
    for (dir, spelled) in std::iter::once(&root).chain(tracked.iter().filter(|(d, _)| !d.is_empty())) {
        let Some(folder) = folder_of(workdir, dir) else { continue };
        let order = if *spelled == Some(1) { [1, 0] } else { [0, 1] };
        for name in order.map(|i| NAMES[i]) {
            if !crate::net_path::may_follow(&folder.join(name), home) {
                let shown = crate::repo::shown_path(dir);
                return Some(if shown.is_empty() { name.to_owned() } else { format!("{shown}/{name}") });
            }
        }
    }
    None
}

/// A folder ('/'-separated bytes; empty for the root) in which an index holds a link a volume
/// could take for one of the two files, and which of `NAMES` that entry spells (`named`).
type Tracked = (Vec<u8>, Option<usize>);

/// An index file's size and time when it was read, and its link folders.
type Seen = (u64, SystemTime, Vec<Tracked>);

/// The folders (sorted) in which the index file `file`
/// holds a link a volume could take for one of the two files (`may_name`). Read from the file on
/// its own, so a repository's shared index stays as its caller left it, and remembered until the
/// file's size or time changes; a file modified too recently for those to tell
/// (`repo::stamp_trustworthy`) is read every time. Empty without an index file; None for one
/// that can't be read (libgit2's own read of the work tree then fails by itself).
fn link_folders(file: &Path) -> Option<Vec<Tracked>> {
    static SEEN: Mutex<Option<HashMap<PathBuf, Seen>>> = Mutex::new(None);
    let meta = match std::fs::metadata(file) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };
    let settled = meta.modified().ok().filter(|t| crate::repo::stamp_trustworthy(Some(*t), SystemTime::now()));
    if let Some(time) = settled
        && let Ok(seen) = SEEN.lock()
        && let Some((len, at, folders)) = seen.as_ref().and_then(|m| m.get(file))
        && (*len, *at) == (meta.len(), time)
    {
        return Some(folders.clone());
    }
    let _span = tracing::debug_span!("tree_files.link_folders.read").entered();
    let links = match crate::repo::index_walk::entries_of_file(file, |mode, _| mode == LINK) {
        Some(found) => found,
        // A split or sparse index, or one the walk doesn't read: libgit2 knows them.
        None => git2::Index::open(file).ok()?.iter().filter(|e| e.mode == LINK).map(|e| (e.mode, e.path)).collect(),
    };
    let mut folders: BTreeSet<Tracked> = BTreeSet::new();
    for (_, path) in &links {
        let (dir, name) = match path.iter().rposition(|b| *b == b'/') {
            Some(at) => (&path[..at], &path[at + 1..]),
            None => (&path[..0], &path[..]),
        };
        if may_name(name) {
            folders.insert((dir.to_vec(), named(name)));
        }
    }
    let folders: Vec<Tracked> = folders.into_iter().collect();
    if let Some(time) = settled
        && let Ok(mut seen) = SEEN.lock()
    {
        let map = seen.get_or_insert_with(HashMap::new);
        if map.len() >= MAX_REMEMBERED {
            map.clear();
        }
        map.insert(file.to_path_buf(), (meta.len(), time, folders.clone()));
    }
    Some(folders)
}

/// Whether some volume could take the index name `name` (one path part) for `.gitattributes` or
/// `.gitignore`. Wider than any volume's rules, since the folder is asked afterwards: the name
/// in any letter case with anything after it (trailing dots and spaces, a stream name), a short
/// 8.3 name (`~`), and any name with a byte outside ASCII (characters a volume ignores or folds).
fn may_name(name: &[u8]) -> bool {
    !name.is_ascii() || name.contains(&b'~') || named(name).is_some()
}

/// Which of `NAMES` the index name `name` spells: the name in any letter case, with anything
/// after it. None for the other names a volume could take for one (`may_name`).
fn named(name: &[u8]) -> Option<usize> {
    let lower = name.to_ascii_lowercase();
    NAMES.iter().position(|n| lower.starts_with(n.as_bytes()))
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
