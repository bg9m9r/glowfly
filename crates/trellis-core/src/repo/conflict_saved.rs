//! The picks a conflicted working file saved with markers holds (`conflict_restore`), read when
//! the conflict view opens it, so a file saved part way comes back as it was left.

use super::attrs::content_conversion;
use super::conflict::{ConflictContent, ConflictFile};
use super::markers::read_text;
use crate::conflict_restore::{SavedPicks, restore};
use git2::{AttrCheckFlags, AttrValue, Repository};
use std::path::Path;

/// Working files larger than this are not read back for picks.
pub const RESTORE_MAX: u64 = 4 * 1024 * 1024;

/// What the working file of the conflict `file` holds beyond its markers, if anything. `None`
/// for anything but text, a working file that isn't a regular text file (links are not
/// followed) of at most `RESTORE_MAX` bytes in UTF-8, one git converts (a `filter`,
/// `working-tree-encoding` or `ident` attribute: its bytes aren't the stages' text), or one that
/// doesn't match the stages (`conflict_restore::restore`).
pub fn saved_picks(workdir: &Path, file: &ConflictFile) -> Option<SavedPicks> {
    saved_picks_with_limit(workdir, file, RESTORE_MAX)
}

/// `saved_picks` reading files of at most `max` bytes.
pub(super) fn saved_picks_with_limit(workdir: &Path, file: &ConflictFile, max: u64) -> Option<SavedPicks> {
    if !matches!(file.content, ConflictContent::Text(_)) {
        return None;
    }
    {
        // Closed before the file is read (Windows locks files that are held open).
        let repo = Repository::open(workdir).ok()?;
        if content_conversion(&repo, &file.path).is_some() || ident(&repo, &file.path) {
            return None;
        }
    }
    let text = String::from_utf8(read_text(workdir, &file.path, max)?).ok()?;
    restore(file, &text)
}

/// Whether git expands `$Id$` in `path` (the `ident` attribute).
fn ident(repo: &Repository, path: &str) -> bool {
    let raw = repo.get_attr_bytes(Path::new(path), "ident", AttrCheckFlags::FILE_THEN_INDEX).ok().flatten();
    matches!(AttrValue::from_bytes(raw), AttrValue::True)
}
