//! Temporary files that a download stopped or killed before it finished left in the Downloads
//! folder, and removing them at the next start or download. Only Glowfly's own: persist's
//! temporary-file pattern for an update file's name, a regular file, untouched for an hour.
//!
//! A start looks in Downloads only after a download was noted in Glowfly's own folder
//! ([`note_download_started`]), until a look finds nothing left: on macOS the first read of
//! Downloads asks the user for access, which a start must never bring up by itself.
//!
//! A download still running rewrites its temporary file at least once a minute (its read
//! timeout), so it is never that old: one running in another Glowfly is left alone.

use super::{asset_name_ok, split_extension, staging};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The note in Glowfly's own folder that a download into Downloads started.
const STARTED_NOTE: &str = "update-download-started";

/// Notes in `own` (a folder of Glowfly's own) that a download into Downloads is starting, so
/// the next start removes what it may leave ([`tidy_downloads`]). Errors are ignored: the next
/// download clears its leftovers too.
pub fn note_download_started(own: &Path) {
    let _ = crate::persist::atomic_write(&own.join(STARTED_NOTE), b"");
}

/// At the start: removes the temporary files stopped downloads left in the folder `downloads`
/// names, only when a download was noted in `own`. Without that note `downloads` isn't even
/// asked for. The note is forgotten once the folder holds none of those files, or can't be read.
/// Returns how many were removed.
pub fn tidy_downloads(own: &Path, downloads: impl FnOnce() -> Option<PathBuf>, now: SystemTime) -> usize {
    let note = own.join(STARTED_NOTE);
    if std::fs::symlink_metadata(&note).is_err() {
        return 0;
    }
    let (removed, left) = downloads().and_then(|dir| sweep(&dir, now)).unwrap_or((0, 0));
    if left == 0 {
        let _ = crate::persist::remove_file(&note);
    }
    removed
}

/// Removes the temporary files of stopped or killed downloads from `dir` (see the module doc);
/// errors are ignored. Returns how many were removed.
pub(super) fn remove_stopped_downloads(dir: &Path, now: SystemTime) -> usize {
    sweep(dir, now).map_or(0, |(removed, _)| removed)
}

/// [`remove_stopped_downloads`], returning how many were removed and how many are left (still
/// fresh or not removable); None when `dir` can't be read.
fn sweep(dir: &Path, now: SystemTime) -> Option<(usize, usize)> {
    let list = std::fs::read_dir(dir).ok()?;
    let (mut removed, mut left) = (0, 0);
    for entry in list.flatten() {
        if !is_download_temp(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        // Not the entry's own metadata: on Windows that comes from the folder listing, which
        // lags behind a file that is open and being written.
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if !meta.is_file() || meta.file_type().is_symlink() {
            continue;
        }
        let old = meta.modified().is_ok_and(|m| staging::stale(m, now));
        if old && crate::persist::remove_file(&path).is_ok() {
            removed += 1;
        } else {
            left += 1;
        }
    }
    Some((removed, left))
}

/// Whether `file_name` is persist's temporary file for an update file's name.
pub(super) fn is_download_temp(file_name: &str) -> bool {
    crate::persist::temp_target(file_name).is_some_and(is_update_file_name)
}

/// Whether `name` is the name of a release file Glowfly downloads (`glowfly-…`), as it is or
/// with the ` (n)` that `free_name` adds before the extension.
fn is_update_file_name(name: &str) -> bool {
    if !name.starts_with("glowfly-") {
        return false;
    }
    if asset_name_ok(name) {
        return true;
    }
    let (stem, ext) = split_extension(name);
    let Some(base) = stem
        .strip_suffix(')')
        .and_then(|s| s.rsplit_once(" ("))
        .and_then(|(base, n)| (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then_some(base))
    else {
        return false;
    };
    asset_name_ok(&format!("{base}{ext}"))
}

#[cfg(test)]
#[path = "leftovers_tests.rs"]
mod tests;
