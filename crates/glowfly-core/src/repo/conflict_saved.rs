//! The picks a conflicted working file saved with markers holds (`conflict_restore`), read when
//! the conflict view opens it, so a file saved part way comes back as it was left.

use super::attrs::content_conversion;
use super::conflict::{ConflictContent, ConflictFile};
use super::markers::read_text;
use crate::conflict_restore::{ReadBack, SavedPicks, read_back};
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
    match working_copy_with_limit(workdir, file, max) {
        WorkingCopy::Saved(s) => Some(s),
        _ => None,
    }
}

/// How the working file of a conflict stands against its stages.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum WorkingCopy {
    /// Exactly as git wrote it.
    Untouched,
    /// Picks saved with markers, or text typed in.
    Saved(SavedPicks),
    /// Its markers can't be matched with the conflict.
    Unmatched,
    /// Git converts the file (the attribute that says so): the bytes aren't the stages' text.
    Converted(String),
    /// Not text, or not a regular text file git left there that can be read.
    Unreadable,
}

/// The state of the working file of `file`, reading files of at most `RESTORE_MAX` bytes.
pub(super) fn working_copy(workdir: &Path, file: &ConflictFile) -> WorkingCopy {
    working_copy_with_limit(workdir, file, RESTORE_MAX)
}

/// `working_copy` reading files of at most `max` bytes.
pub(super) fn working_copy_with_limit(workdir: &Path, file: &ConflictFile, max: u64) -> WorkingCopy {
    if !matches!(file.content, ConflictContent::Text(_)) {
        return WorkingCopy::Unreadable;
    }
    {
        // Closed before the file is read (Windows locks files that are held open).
        let Ok(repo) = Repository::open(workdir) else { return WorkingCopy::Unreadable };
        if let Some(attr) = content_conversion(&repo, &file.path) {
            return WorkingCopy::Converted(attr);
        }
        if ident(&repo, &file.path) {
            return WorkingCopy::Converted("ident".into());
        }
    }
    let Some(text) = read_text(workdir, &file.path, max).and_then(|b| String::from_utf8(b).ok()) else {
        return WorkingCopy::Unreadable;
    };
    match read_back(file, &text) {
        ReadBack::Untouched => WorkingCopy::Untouched,
        ReadBack::Saved(s) => WorkingCopy::Saved(s),
        ReadBack::Unmatched => WorkingCopy::Unmatched,
    }
}

/// Whether git expands `$Id$` in `path` (the `ident` attribute).
fn ident(repo: &Repository, path: &str) -> bool {
    let raw = repo.get_attr_bytes(Path::new(path), "ident", AttrCheckFlags::FILE_THEN_INDEX).ok().flatten();
    matches!(AttrValue::from_bytes(raw), AttrValue::True)
}
