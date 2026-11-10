//! The work tree's `.gitmodules`, read the way git reads it: a regular file (never through a
//! link) whose `[include]` and `[includeIf]` sections are not followed. libgit2 reads the file as
//! an ordinary config file and follows them, in its own submodule code too, so a file libgit2
//! would read differently from git (`Gitmodules::Unread`) is never opened as a config here, and
//! callers keep libgit2 from the submodules of such a repository (`git_file::submodules_unread`).

use crate::config_includes::read_file_bounded;
use std::path::Path;

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

#[cfg(test)]
#[path = "gitmodules_tests.rs"]
mod tests;
