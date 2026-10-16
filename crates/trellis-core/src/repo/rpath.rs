//! Repository paths as git stores them: bytes, '/'-separated, usually but not always UTF-8.
//!
//! Every path read from libgit2 or git's output goes through here. git2's `DiffFile::path()`
//! (and every other accessor that returns a `&Path`) panics on Windows when the bytes aren't
//! UTF-8, so only the `*_bytes()` accessors are used, and a name that isn't UTF-8 becomes a
//! row Trellis shows but won't act on (`Hazard::NotUtf8`), never a lossy path that could name
//! another file.

use super::{FileEntry, Hazard};
use git2::{DiffDelta, DiffFile};

/// The row for the path `bytes` with `status`.
pub(super) fn entry(bytes: &[u8], status: char) -> FileEntry {
    match std::str::from_utf8(bytes) {
        Ok(path) => FileEntry::new(path, status),
        Err(_) => FileEntry { path: String::from_utf8_lossy(bytes).into_owned(), status, hazard: Some(Hazard::NotUtf8(bytes.to_vec())) },
    }
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

/// The path a delta is listed under: its new path, or the old one for a deletion.
pub(super) fn delta_bytes<'a>(delta: &DiffDelta<'a>) -> &'a [u8] {
    delta.new_file().path_bytes().or(delta.old_file().path_bytes()).unwrap_or_default()
}
