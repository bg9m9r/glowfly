//! Old programs a portable or AppImage swap left beside this copy. The swap moves the running
//! program to `<name>.bak` (`persist::move_aside`, `<name>.bak-<n>` when that is taken) and the
//! helper deletes it once Glowfly has quit; on Windows that fails while a second window still
//! runs the old program, so the next start tries again ([`remove_unused`]).
//!
//! Only those exact names beside the program are touched, only regular files (never a link,
//! which is not followed), and only when the process list could be read and no process runs the
//! file. Windows also refuses to delete a program that is running, which backs up the check.

use crate::native_path::shown;
use std::path::{Path, PathBuf};

/// Whether `name` is a backup `persist::move_aside` makes of a file called `program`:
/// `<program>.bak`, or `<program>.bak-<n>` with `n` of 2 or more.
pub fn is_backup_name(program: &str, name: &str) -> bool {
    let Some(rest) = name.strip_prefix(program).and_then(|r| r.strip_prefix(".bak")) else { return false };
    match rest.strip_prefix('-') {
        None => rest.is_empty(),
        Some(n) => !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()) && n != "1",
    }
}

/// The backups of `program` in its folder: regular files (not links) named as
/// [`is_backup_name`] says.
pub fn backups_of(program: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (program.parent(), program.file_name().and_then(|n| n.to_str())) else { return Vec::new() };
    let Ok(list) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = list
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| is_backup_name(name, n)))
        .map(|e| e.path())
        .filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file()))
        .collect();
    out.sort();
    out
}

/// Whether one of `running` (every process's program file) is `file`: the same spelling, or,
/// for a path with the same file name in any case, the same file on disk.
pub fn runs(running: &[PathBuf], file: &Path) -> bool {
    let name = file.file_name().map(|n| n.to_string_lossy().to_lowercase());
    running.iter().any(|p| {
        crate::paths::same_spelling(p, file)
            || (p.file_name().map(|n| n.to_string_lossy().to_lowercase()) == name && crate::paths::same_folder(p, file))
    })
}

/// What [`remove_unused`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cleared {
    pub removed: Vec<PathBuf>,
    /// Kept: still running, or the process list couldn't be read.
    pub running: Vec<PathBuf>,
    /// Removing failed, with why.
    pub failed: Vec<String>,
}

/// Removes the backups of each of `programs` that no process runs. `list` gives every
/// process's program file (`terminal::program_paths`); when it can't (None), nothing is removed.
pub fn remove_unused(programs: &[PathBuf], list: impl FnOnce() -> Option<Vec<PathBuf>>) -> Cleared {
    let mut out = Cleared::default();
    let found: Vec<PathBuf> = programs.iter().flat_map(|p| backups_of(p)).collect();
    if found.is_empty() {
        return out;
    }
    let Some(running) = list() else {
        out.running = found;
        return out;
    };
    for bak in found {
        if runs(&running, &bak) {
            out.running.push(bak);
            continue;
        }
        match crate::persist::remove_file(&bak) {
            Ok(()) => out.removed.push(bak),
            Err(e) => out.failed.push(format!("{}: {e}", shown(&bak))),
        }
    }
    out
}

#[cfg(test)]
#[path = "backups_tests.rs"]
mod tests;
