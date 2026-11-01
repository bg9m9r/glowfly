//! Repository paths as git stores them: bytes, '/'-separated, usually but not always UTF-8.
//!
//! Every path read from libgit2 or git's output goes through here. git2's `DiffFile::path()`
//! (and every other accessor that returns a `&Path`) panics on Windows when the bytes aren't
//! UTF-8, so only the `*_bytes()` accessors are used, and a name that isn't UTF-8 becomes a
//! row Glowfly shows but won't act on (`Hazard::NotUtf8`), never a lossy path that could name
//! another file.

use super::{FileEntry, Hazard};
use git2::{Diff, DiffDelta, DiffFile};

/// The row for the path `bytes` with `status`.
pub(super) fn entry(bytes: &[u8], status: char) -> FileEntry {
    match std::str::from_utf8(bytes) {
        Ok(path) => FileEntry::new(path, status),
        Err(_) => FileEntry { path: shown(bytes), status, hazard: Some(Hazard::NotUtf8(bytes.to_vec())), lfs: false, renamed_from: None },
    }
}

/// `bytes` for the screen: the UTF-8 parts as they are, each other byte as `\xHH`.
pub(crate) fn shown(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 8);
    for chunk in bytes.utf8_chunks() {
        out.push_str(chunk.valid());
        for b in chunk.invalid() {
            out.push_str(&format!("\\x{b:02X}"));
        }
    }
    out
}

/// The path `bytes` as text, or None when it isn't UTF-8.
pub(super) fn text(bytes: &[u8]) -> Option<String> {
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

/// Whether `file` is exactly `path` (byte for byte: no case folding, no globbing).
pub(super) fn is(file: &DiffFile<'_>, path: &str) -> bool {
    file.path_bytes() == Some(path.as_bytes())
}

/// Whether either side of `delta` is exactly `path`.
pub(super) fn touches(delta: &DiffDelta<'_>, path: &str) -> bool {
    is(&delta.new_file(), path) || is(&delta.old_file(), path)
}

/// The index of `path`'s delta in `diff`: the one listed under it (`delta_bytes`), else one with
/// `path` on its other side (a rename asked for by its old name). A copy's source is listed
/// under its own name, so it is never taken for the copy.
pub(super) fn delta_of(diff: &Diff<'_>, path: &str) -> Option<usize> {
    diff.deltas().position(|d| delta_bytes(&d) == path.as_bytes()).or_else(|| diff.deltas().position(|d| touches(&d, path)))
}

/// `path`'s delta in `diff` (`delta_of`).
pub(super) fn find_delta<'d>(diff: &'d Diff<'_>, path: &str) -> Option<DiffDelta<'d>> {
    diff.get_delta(delta_of(diff, path)?)
}

/// The path a delta is listed under: its new path, or the old one for a deletion.
pub(super) fn delta_bytes<'a>(delta: &DiffDelta<'a>) -> &'a [u8] {
    delta.new_file().path_bytes().or(delta.old_file().path_bytes()).unwrap_or_default()
}

/// Whether the path `bytes` is inside the folder `folder` (byte for byte, never case-folded).
pub(super) fn under(bytes: &[u8], folder: &str) -> bool {
    bytes.strip_prefix(folder.as_bytes()).is_some_and(|rest| rest.first() == Some(&b'/'))
}

#[cfg(test)]
#[path = "copy_source_tests.rs"]
mod copy_source_tests;
