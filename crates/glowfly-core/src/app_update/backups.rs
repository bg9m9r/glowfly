//! Old programs a portable or AppImage swap left beside this copy. The swap moves the running
//! program to `<name>.bak` (`persist::move_aside`, `<name>.bak-<n>` when that is taken),
//! records it beside it ([`record`]: `<backup>.record`, its size and SHA-256), and the helper
//! deletes both once Glowfly has quit; on Windows that fails while a second window still runs
//! the old program, so the next start tries again ([`remove_unused`]).
//!
//! Only a backup Glowfly recorded is touched, and only while it still has the recorded size and
//! SHA-256, so a `glowfly.bak` the user made (or one an older release left without a record)
//! stays. Only those exact names beside the program count, only regular files (never a link,
//! which is not followed), and only when the process list could be read and no process runs
//! the file. Windows also refuses to delete a program that is running, which backs up the check.

use crate::native_path::shown;
use crate::persist::{self, WriteOptions};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

/// The format of a record this build writes and reads.
const RECORD_FORMAT: u32 = 1;
/// A record is a few dozen bytes; anything longer isn't one.
const RECORD_MAX: u64 = 4096;

/// What Glowfly wrote about a backup it made.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct Record {
    format: u32,
    size: u64,
    /// Lower-case hex.
    sha256: String,
}

/// The record beside `backup`: `<backup>.record`.
pub fn record_path(backup: &Path) -> PathBuf {
    let mut name = backup.file_name().unwrap_or_default().to_os_string();
    name.push(".record");
    backup.with_file_name(name)
}

fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The size and SHA-256 of the regular file at `path` (not through a link).
fn fingerprint(path: &Path) -> Option<(u64, String)> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let size = u64::try_from(bytes.len()).ok()?;
    (size == meta.len()).then(|| (size, hex(&super::sha256(&bytes))))
}

/// Records `backup`, a file the swap just moved aside, so the next start may remove it. Returns
/// the record's path.
pub fn record(backup: &Path) -> Result<PathBuf, String> {
    let (size, sha256) = fingerprint(backup).ok_or_else(|| format!("Couldn't read {}", shown(backup)))?;
    let json = serde_json::to_vec(&Record { format: RECORD_FORMAT, size, sha256 }).map_err(|e| e.to_string())?;
    let path = record_path(backup);
    persist::write_file(&path, &json, WriteOptions::default()).map_err(|e| format!("Couldn't write {}: {e}", shown(&path)))?;
    Ok(path)
}

/// Whether `backup` has a record (a regular file, not a link) and still matches it: the same
/// size, then the same SHA-256.
pub fn recorded(backup: &Path) -> bool {
    let path = record_path(backup);
    let is_file = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file() && m.len() <= RECORD_MAX);
    let mut text = Vec::new();
    let read = is_file && std::fs::File::open(&path).and_then(|f| f.take(RECORD_MAX).read_to_end(&mut text)).is_ok();
    let Some(rec) = read.then(|| serde_json::from_slice::<Record>(&text).ok()).flatten() else { return false };
    if rec.format != RECORD_FORMAT || std::fs::symlink_metadata(backup).map(|m| m.len()).ok() != Some(rec.size) {
        return false;
    }
    fingerprint(backup).is_some_and(|(size, sha)| size == rec.size && sha == rec.sha256)
}

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
    /// Kept: no record, or changed since it was recorded (not Glowfly's to remove).
    pub unrecorded: Vec<PathBuf>,
    /// Kept: still running, or the process list couldn't be read.
    pub running: Vec<PathBuf>,
    /// Removing failed, with why.
    pub failed: Vec<String>,
}

/// Removes the recorded backups of each of `programs` that no process runs, with their records.
/// `list` gives every process's program file (`terminal::program_paths`); when it can't (None),
/// nothing is removed.
pub fn remove_unused(programs: &[PathBuf], list: impl FnOnce() -> Option<Vec<PathBuf>>) -> Cleared {
    let mut out = Cleared::default();
    let (found, unrecorded): (Vec<PathBuf>, Vec<PathBuf>) = programs.iter().flat_map(|p| backups_of(p)).partition(|b| recorded(b));
    out.unrecorded = unrecorded;
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
        match persist::remove_file(&bak).and_then(|()| persist::remove_file(&record_path(&bak))) {
            Ok(()) => out.removed.push(bak),
            Err(e) => out.failed.push(format!("{}: {e}", shown(&bak))),
        }
    }
    out
}

#[cfg(test)]
#[path = "backups_tests.rs"]
mod tests;
