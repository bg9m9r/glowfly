//! The object stores libgit2 reaches when it reads a repository's objects: its own `objects/`
//! and the alternates its `info/alternates` names, followed as libgit2 does (up to depth 5),
//! each checked with `net_path` before anything past the spelling is touched.
//!
//! libgit2 reads the alternates file of the repository's own store (depth 0) and of each store
//! it names, down to depth 5; the stores a depth-5 file names (depth 6) are still looked at, but
//! their own alternates file is never read. A line starting with `.` is joined to the `objects`
//! folder holding the file, any other line is used as it is: absolute, or relative to the
//! process's own folder, which a repository's content may not choose, so such a line is refused.

use crate::config_includes::read_file_bounded;
use crate::net_path::may_follow;
use std::path::{Path, PathBuf};

/// libgit2's `GIT_ALTERNATES_MAX_DEPTH`: the deepest store whose alternates file is read.
const MAX_DEPTH: usize = 5;
/// The most object stores one repository may reach before it is refused.
const MAX_STORES: usize = 32;
/// The largest alternates file read; a larger one is refused.
const ALTERNATES_MAX: u64 = 64 * 1024;

/// Whether libgit2 reading the objects of the repository whose common dir is `common` (already
/// allowed from `home`) stays clear of refused places: Err with the first one found.
pub(super) fn reachable(common: &Path, home: &Path) -> Result<(), PathBuf> {
    let mut seen = Vec::new();
    store(&common.join("objects"), common, home, 0, &mut seen)
}

/// Checks the object store `objects`, with `base` the allowed folder it is reached from, and
/// the alternates it names.
fn store(objects: &Path, base: &Path, home: &Path, depth: usize, seen: &mut Vec<PathBuf>) -> Result<(), PathBuf> {
    if !may_follow(objects, base) {
        return Err(objects.to_path_buf());
    }
    if seen.iter().any(|s| crate::paths::same_spelling(s, objects)) {
        return Ok(());
    }
    if seen.len() >= MAX_STORES {
        return Err(objects.to_path_buf());
    }
    seen.push(objects.to_path_buf());
    // libgit2 lists the loose fan-out folders, `pack/*.idx` and `*.pack` and reads
    // `info/commit-graph`: a link among them to a refused place is refused, never followed.
    for dir in [objects.to_path_buf(), objects.join("info"), objects.join("pack")] {
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        for e in read.flatten() {
            if e.file_type().is_ok_and(|t| t.is_symlink()) && !may_follow(&e.path(), &dir) {
                return Err(e.path());
            }
        }
    }
    // libgit2 never reads a deeper store's alternates file.
    if depth > MAX_DEPTH {
        return Ok(());
    }
    let file = objects.join("info").join("alternates");
    if !may_follow(&file, objects) {
        return Err(file);
    }
    if std::fs::symlink_metadata(&file).is_err() {
        return Ok(());
    }
    // Too large, not a regular file, or unreadable.
    let Some(bytes) = read_file_bounded(&file, ALTERNATES_MAX) else { return Err(file) };
    // Read as a C string, split as libgit2's `git__strtok(.., "\r\n")` does.
    for line in super::up_to_nul(&bytes).split(|b| *b == b'\r' || *b == b'\n') {
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        let Ok(line) = std::str::from_utf8(line) else { return Err(file) };
        let target = if line.starts_with('.') {
            objects.join(line)
        } else if Path::new(line).is_absolute() {
            PathBuf::from(line)
        } else {
            // Relative to the process's own folder in libgit2 (a quoted line too).
            return Err(objects.join(line));
        };
        // Not under any folder already allowed: checked from `home`.
        store(&target, home, home, depth + 1, seen)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "alternates_tests.rs"]
mod tests;
