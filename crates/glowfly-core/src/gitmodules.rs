//! The work tree's `.gitmodules`, read the way git reads it: a regular file (never through a
//! link) whose `[include]` and `[includeIf]` sections are not followed. libgit2 reads the file as
//! an ordinary config file and follows them, in its own submodule code too, so a file libgit2
//! would read differently from git (`Gitmodules::Unread`) is never opened as a config here, and
//! callers keep libgit2 from the submodules of such a repository (`git_file::submodules_unread`).

use crate::config_includes::read_file_bounded;
use crate::git_file::{Found, other_repo, work_tree_is};
use git2::Repository;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The largest `.gitmodules` read.
pub(crate) const MAX_BYTES: u64 = 1 << 20;

/// What the work tree holds as `.gitmodules`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gitmodules {
    Absent,
    /// A regular file of at most `MAX_BYTES` with no include section.
    Plain,
    /// A link, something that isn't a regular file, a file over `MAX_BYTES` or one that can't be
    /// read, or a file with an include section: its values aren't used.
    Unread,
}

/// What `workdir` holds as `.gitmodules`. One stat, and one bounded read of a regular file.
pub(crate) fn kind(workdir: &Path) -> Gitmodules {
    let file = workdir.join(".gitmodules");
    match std::fs::symlink_metadata(&file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Gitmodules::Absent,
        Ok(m) if m.file_type().is_file() && m.len() <= MAX_BYTES => match read_file_bounded(&file, MAX_BYTES) {
            Some(bytes) if !names_include_section(&bytes) => Gitmodules::Plain,
            _ => Gitmodules::Unread,
        },
        _ => Gitmodules::Unread,
    }
}

/// `workdir`'s `.gitmodules` as a config, when it is `Gitmodules::Plain`.
pub(crate) fn open(workdir: &Path) -> Option<git2::Config> {
    (kind(workdir) == Gitmodules::Plain).then(|| git2::Config::open(&workdir.join(".gitmodules")).ok()).flatten()
}

/// Whether `bytes` hold `[` directly followed by `include` in any letter case, anywhere: the
/// only way a config file starts a section named `include`, `includeIf` or anything else
/// beginning so (git's and libgit2's syntax take the name's bytes as written right after the
/// `[`, with no space, quote or escape before or inside it, on one line). Deliberately wider
/// than the syntax: where the `[` stands (after a continued line, in a value or a comment) isn't
/// looked at, and a byte that isn't ASCII stands for any letter, since libgit2 lowers the name
/// with the C library's `tolower`, which a locale can widen.
pub(crate) fn names_include_section(bytes: &[u8]) -> bool {
    const NAME: &[u8] = b"include";
    bytes
        .windows(NAME.len() + 1)
        .any(|w| w[0] == b'[' && w[1..].iter().zip(NAME).all(|(b, n)| !b.is_ascii() || b.to_ascii_lowercase() == *n))
}

/// How deep below the opened repository checked-out submodules are looked at (the trust scan's
/// depth); one deeper counts as unread.
const MAX_DEPTH: usize = 8;
/// The most checked-out submodules looked at in one walk; more count as unread.
const MAX_WALKED: usize = 4096;

/// Whether libgit2 would read a `.gitmodules` differently from git anywhere it reads one for
/// `repo`'s work tree: the work tree's own, or that of a checked-out submodule at any depth
/// (libgit2's status of a submodule compares its work tree, which reads the submodule's own
/// `.gitmodules` when its index holds a gitlink, and so on down). Whatever can't be told counts
/// as unread: a gitlink whose name isn't UTF-8 or isn't a plain relative path inside the work
/// tree, a submodule whose repository names another work tree than its own folder
/// (`git_file::work_tree_is`), one whose index can't be read, one below the first level whose
/// files name a place that may not be followed, and ones past `MAX_DEPTH` or `MAX_WALKED`. Per
/// checked-out submodule: what `git_file::other_repo` looks at, one stat and bounded read of its
/// `.gitmodules`, one stat of its index, and its index read again only when that file changed.
pub(crate) fn unread_anywhere(repo: &Repository) -> bool {
    // A bare repository: libgit2 looks no submodule up without a work tree.
    let Some(workdir) = repo.workdir() else { return false };
    if kind(workdir) == Gitmodules::Unread {
        return true;
    }
    let Ok(mut index) = repo.index() else { return true };
    let _ = index.read(false);
    let links = index.iter().filter(|e| e.mode == 0o160000).map(|e| e.path).collect();
    below(workdir, links, 0, &mut 0)
}

/// Whether a checked-out submodule among `links` (gitlink names in `workdir`'s index), or below
/// one, has an unread `.gitmodules` or can't be told.
fn below(workdir: &Path, mut links: Vec<Vec<u8>>, depth: usize, walked: &mut usize) -> bool {
    // A conflicted gitlink has an entry per side.
    links.dedup();
    for name in links {
        // libgit2 joins the name's bytes to the work tree as they are; a name that can't be
        // joined the same way here can't be looked at.
        let Ok(path) = String::from_utf8(name) else { return true };
        let Ok(folder) = crate::repo::work_path(workdir, &path) else { return true };
        let dirs = match other_repo(&folder, workdir) {
            Found::Repo(d) => d,
            // No `.git` there, or one that names nothing: libgit2 finds no repository to open.
            Found::None => continue,
            // At the first level `git_file::offsite_gitlinks` reports it, and every caller
            // keeps libgit2 from such a submodule by that.
            Found::Refused(_) if depth == 0 => continue,
            Found::Refused(_) => return true,
        };
        *walked += 1;
        if depth + 1 >= MAX_DEPTH || *walked > MAX_WALKED || !work_tree_is(&dirs, &folder) || kind(&folder) == Gitmodules::Unread {
            return true;
        }
        let Some(inner) = gitlinks_of(&dirs.git_dir.join("index")) else { return true };
        if below(&folder, inner, depth + 1, walked) {
            return true;
        }
    }
    false
}

/// An index file's size and time when it was read, and the gitlink names it held.
type Seen = (u64, Option<SystemTime>, Vec<Vec<u8>>);

/// The gitlink names in the index file `file`, sorted as the index is; remembered until the
/// file's size or time changes. Empty without an index file (libgit2 then reads an empty
/// index); None for one that can't be read.
fn gitlinks_of(file: &Path) -> Option<Vec<Vec<u8>>> {
    static SEEN: Mutex<Option<HashMap<PathBuf, Seen>>> = Mutex::new(None);
    let meta = match std::fs::metadata(file) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };
    let stamp = (meta.len(), meta.modified().ok());
    if let Ok(seen) = SEEN.lock()
        && let Some((len, time, links)) = seen.as_ref().and_then(|m| m.get(file))
        && (*len, *time) == stamp
    {
        return Some(links.clone());
    }
    let links: Vec<Vec<u8>> = git2::Index::open(file).ok()?.iter().filter(|e| e.mode == 0o160000).map(|e| e.path).collect();
    if let Ok(mut seen) = SEEN.lock() {
        let map = seen.get_or_insert_with(HashMap::new);
        if map.len() >= MAX_WALKED {
            map.clear();
        }
        map.insert(file.to_path_buf(), (stamp.0, stamp.1, links.clone()));
    }
    Some(links)
}

#[cfg(test)]
#[path = "gitmodules_tests.rs"]
mod tests;
