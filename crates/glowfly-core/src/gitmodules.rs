//! The work tree's `.gitmodules`, read the way git reads it: a regular file (never through a
//! link) whose `[include]` and `[includeIf]` sections are not followed. libgit2 reads the file as
//! an ordinary config file and follows them, in its own submodule code too, so a file libgit2
//! would read differently from git (`Gitmodules::Unread`) is never opened as a config here, and
//! callers keep libgit2 from the submodules of such a repository (`git_file::submodules_unread`).

use crate::config_includes::read_file_bounded;
use crate::git_file::{Found, dirs_of, other_repo};
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
/// `.gitmodules` when its index holds a gitlink, and so on down). A submodule below the first
/// level whose files name a place that may not be followed counts too, as do ones past
/// `MAX_DEPTH` or `MAX_WALKED`, which can't be told. Per checked-out submodule: one stat and
/// bounded read of its `.gitmodules`, one stat of its index, and its index read again only when
/// that file changed.
pub(crate) fn unread_anywhere(repo: &Repository) -> bool {
    let Some(workdir) = repo.workdir() else { return false };
    if kind(workdir) == Gitmodules::Unread {
        return true;
    }
    let Ok(mut index) = repo.index() else { return false };
    let _ = index.read(false);
    let links = index.iter().filter(|e| e.mode == 0o160000).filter_map(|e| String::from_utf8(e.path).ok()).collect();
    below(workdir, links, 0, &mut 0)
}

/// Whether a checked-out submodule among `links` (gitlink paths in `workdir`'s index), or below
/// one, has an unread `.gitmodules`.
fn below(workdir: &Path, mut links: Vec<String>, depth: usize, walked: &mut usize) -> bool {
    links.dedup();
    for path in links {
        // A name that isn't a plain path inside the work tree has no folder there.
        let Ok(folder) = crate::repo::work_path(workdir, &path) else { continue };
        // The first level was looked at whole by `git_file::offsite_gitlinks`; below it, what
        // libgit2 would open is checked here.
        let found = if depth == 0 { dirs_of(&folder, workdir) } else { other_repo(&folder, workdir) };
        let dirs = match found {
            Found::Repo(d) => d,
            Found::None => continue,
            Found::Refused(_) if depth == 0 => continue,
            Found::Refused(_) => return true,
        };
        *walked += 1;
        if depth + 1 >= MAX_DEPTH || *walked > MAX_WALKED || kind(&folder) == Gitmodules::Unread {
            return true;
        }
        if below(&folder, gitlinks_of(&dirs.git_dir.join("index")), depth + 1, walked) {
            return true;
        }
    }
    false
}

/// An index file's size and time when it was read, and the gitlink paths it held.
type Seen = (u64, Option<SystemTime>, Vec<String>);

/// The gitlink paths in the index file `file`, sorted as the index is; remembered until the
/// file's size or time changes. Empty for a missing or unreadable index.
fn gitlinks_of(file: &Path) -> Vec<String> {
    static SEEN: Mutex<Option<HashMap<PathBuf, Seen>>> = Mutex::new(None);
    let Ok(meta) = std::fs::metadata(file) else { return Vec::new() };
    let stamp = (meta.len(), meta.modified().ok());
    if let Ok(seen) = SEEN.lock()
        && let Some((len, time, links)) = seen.as_ref().and_then(|m| m.get(file))
        && (*len, *time) == stamp
    {
        return links.clone();
    }
    let links: Vec<String> = match git2::Index::open(file) {
        Ok(index) => index.iter().filter(|e| e.mode == 0o160000).filter_map(|e| String::from_utf8(e.path).ok()).collect(),
        Err(_) => Vec::new(),
    };
    if let Ok(mut seen) = SEEN.lock() {
        let map = seen.get_or_insert_with(HashMap::new);
        if map.len() >= MAX_WALKED {
            map.clear();
        }
        map.insert(file.to_path_buf(), (stamp.0, stamp.1, links.clone()));
    }
    links
}

#[cfg(test)]
#[path = "gitmodules_tests.rs"]
mod tests;
