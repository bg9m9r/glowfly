//! Temporary files that a download stopped or killed before it finished left in the Downloads
//! folder, and removing them at the next start or download. Only Glowfly's own: persist's
//! temporary-file pattern for an update file's name, a regular file, untouched for an hour.
//!
//! A download still running rewrites its temporary file at least once a minute (its read
//! timeout), so it is never that old: one running in another Glowfly is left alone.

use super::{asset_name_ok, split_extension, staging};
use std::path::Path;
use std::time::SystemTime;

/// Removes the temporary files of stopped or killed downloads from `dir` (see the module doc);
/// errors are ignored. Returns how many were removed.
pub fn remove_stopped_downloads(dir: &Path, now: SystemTime) -> usize {
    let Ok(list) = std::fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for entry in list.flatten() {
        if !is_download_temp(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        // Not the entry's own metadata: on Windows that comes from the folder listing, which
        // lags behind a file that is open and being written.
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let old = meta.modified().is_ok_and(|m| staging::stale(m, now));
        if meta.is_file() && !meta.file_type().is_symlink() && old && crate::persist::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
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
