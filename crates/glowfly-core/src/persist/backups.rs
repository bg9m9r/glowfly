//! Backups beside a file: `<name>.bak`, then `<name>.bak-2`, `<name>.bak-3`, …, each numbered one
//! past the highest backup already there, so a higher number is always a newer copy. A file
//! moved aside ([`move_aside`]) keeps its old modified time, which is why age is told by the
//! number and not by the clock. [`prune_backups`] keeps the newest few of a config file's copies
//! at start and deletes the rest; only regular files with exactly those names are counted or
//! removed, never a link, a folder or any other file in the folder.

use super::{WriteOptions, write_file};
use std::path::{Path, PathBuf};

/// How many `<file>.bak*` copies each of Glowfly's config files keeps (pruned at start).
pub const KEPT_BACKUPS: usize = 3;

/// The number of `candidate` as a backup of a file called `file`: 1 for `<file>.bak`, `n` for
/// `<file>.bak-<n>` (`n` of 2 or more, ASCII digits, no leading zero), and `None` for any other
/// name. Names are matched exactly, case included.
pub fn backup_number(file: &str, candidate: &str) -> Option<u64> {
    let rest = candidate.strip_prefix(file)?.strip_prefix(".bak")?;
    let Some(n) = rest.strip_prefix('-') else { return rest.is_empty().then_some(1) };
    if n.is_empty() || n.starts_with('0') || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok().filter(|n| *n >= 2)
}

/// The name of `path`'s file, when it has one in Unicode.
fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|n| n.to_str())
}

/// Every entry beside `path` named as a backup of it, with its number, whatever it is (file,
/// folder or link). A folder that can't be read has none.
fn named_backups(path: &Path) -> Vec<(u64, PathBuf)> {
    let (Some(dir), Some(name)) = (path.parent(), file_name(path)) else { return Vec::new() };
    let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
    let Ok(list) = std::fs::read_dir(dir) else { return Vec::new() };
    list.flatten()
        .filter_map(|e| {
            let n = e.file_name().to_str().and_then(|c| backup_number(name, c))?;
            Some((n, path.with_file_name(e.file_name())))
        })
        .collect()
}

/// The backups of `path` that pruning may remove: regular files only (not through a link).
fn backups(path: &Path) -> Vec<(u64, PathBuf)> {
    named_backups(path).into_iter().filter(|(_, p)| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file())).collect()
}

/// The backup name for a new copy of `path`: one past the highest backup number beside it
/// (`<name>.bak` when there is none), skipping any name something already has.
fn free_backup_name(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let first = named_backups(path).into_iter().map(|(n, _)| n).max().map_or(1, |n| n.saturating_add(1));
    (first..)
        .map(|i| path.with_file_name(if i == 1 { format!("{name}.bak") } else { format!("{name}.bak-{i}") }))
        .find(|p| p.symlink_metadata().is_err())
        .expect("an unused backup name")
}

/// Moves `path` aside to a new backup name (`<name>.bak`, `<name>.bak-<n>`: see the module doc)
/// and returns it.
pub fn move_aside(path: &Path) -> std::io::Result<PathBuf> {
    let backup = free_backup_name(path);
    std::fs::rename(path, &backup)?;
    Ok(backup)
}

/// Writes `bytes` (what was read from `path`) to the backup name `move_aside` would pick, leaving
/// `path` where it is, and returns that name. The copy is private (`WriteOptions::private`), as
/// the files kept this way are.
pub fn keep_copy(path: &Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let backup = free_backup_name(path);
    write_file(&backup, bytes, WriteOptions { private: true, ..WriteOptions::default() })?;
    Ok(backup)
}

/// Deletes all but the `keep` newest (highest-numbered) backups of `path` and returns the ones
/// deleted. A backup that can't be deleted (another program holds it) is logged and skipped.
pub fn prune_backups(path: &Path, keep: usize) -> Vec<PathBuf> {
    let mut found = backups(path);
    found.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    let mut removed = Vec::new();
    for (_, old) in found.into_iter().skip(keep) {
        match super::remove_file(&old) {
            Ok(()) => removed.push(old),
            Err(e) => tracing::warn!("could not remove the old backup {}: {e}", crate::native_path::shown(&old)),
        }
    }
    removed
}

#[cfg(test)]
#[path = "backups_tests.rs"]
mod tests;
