//! Finding the Git repositories under the folders a user keeps them in (the Find repositories
//! dialog's project folders). Only folder listings and each folder's `.git` entry are read:
//! nothing is opened with libgit2, no git runs, and nothing in the repositories runs.
//!
//! The walk is breadth-first under each root, at most `MAX_DEPTH` levels down and `MAX_FOLDERS`
//! folders in all. It never follows links or junctions, never descends into a repository it
//! found (so nested repositories and submodules aren't listed), and skips folders that hold no
//! repositories of their own (`.git`, `node_modules`, the Windows recycle bin and volume data).

use crate::paths;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// How many levels below a root the scan looks (the root itself is level 0).
pub const MAX_DEPTH: usize = 4;
/// How many folders one scan looks at before it stops.
pub const MAX_FOLDERS: usize = 20_000;

/// Folders never entered, compared ignoring ASCII case. Skipping one too many is harmless.
const SKIPPED: [&str; 4] = [".git", "node_modules", "$RECYCLE.BIN", "System Volume Information"];

/// A repository found under `root` (the project folder it was found from).
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub root: PathBuf,
    pub path: PathBuf,
}

/// What one scan found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanReport {
    /// Each repository once (by spelling), under the first root that reached it, sorted by
    /// folder name ignoring case, then by path.
    pub found: Vec<Found>,
    /// Roots that are missing or couldn't be listed, with the reason.
    pub unreadable: Vec<(PathBuf, String)>,
    /// The scan stopped at `MAX_FOLDERS` before it had looked everywhere.
    pub truncated: bool,
}

/// Scans `roots` with the default limits.
pub fn scan(roots: &[PathBuf]) -> ScanReport {
    scan_with(roots, MAX_DEPTH, MAX_FOLDERS)
}

/// Scans `roots`, looking at most `max_depth` levels down and at `max_folders` folders in all.
pub fn scan_with(roots: &[PathBuf], max_depth: usize, max_folders: usize) -> ScanReport {
    let mut report = ScanReport::default();
    let mut looked = 0usize;
    'roots: for root in roots {
        let mut queue = VecDeque::from([(root.clone(), 0usize)]);
        while let Some((dir, depth)) = queue.pop_front() {
            if looked >= max_folders {
                report.truncated = true;
                break 'roots;
            }
            looked += 1;
            if is_repo_folder(&dir) {
                if !report.found.iter().any(|f| paths::same_spelling(&f.path, &dir)) {
                    report.found.push(Found { root: root.clone(), path: dir });
                }
                continue;
            }
            if depth >= max_depth {
                continue;
            }
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                // A subfolder that can't be listed is passed over; only a root is reported.
                Err(e) => {
                    if depth == 0 {
                        report.unreadable.push((root.clone(), e.to_string()));
                    }
                    continue;
                }
            };
            for entry in entries.flatten() {
                // `file_type` doesn't follow links; on Windows a junction is a link too.
                let Ok(kind) = entry.file_type() else { continue };
                let name = entry.file_name();
                if kind.is_symlink() || !kind.is_dir() || skipped(&name) {
                    continue;
                }
                queue.push_back((dir.join(name), depth + 1));
            }
        }
    }
    report.found.sort_by_cached_key(|f| (sort_name(&f.path), f.path.clone()));
    report
}

/// Whether `dir` holds a repository: a `.git` folder with a `HEAD`, or a `.git` file pointing
/// elsewhere (`gitdir: …`, as linked worktrees and submodules have). The `.git` entry itself is
/// not followed when it is a link.
pub fn is_repo_folder(dir: &Path) -> bool {
    let git = dir.join(".git");
    let Ok(meta) = std::fs::symlink_metadata(&git) else { return false };
    if meta.is_dir() {
        return git.join("HEAD").exists();
    }
    meta.is_file() && starts_with_gitdir(&git)
}

fn starts_with_gitdir(file: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 7];
    std::fs::File::open(file).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == b"gitdir:"
}

/// Whether the scan never enters a folder named `name`.
fn skipped(name: &OsStr) -> bool {
    let bytes = name.as_encoded_bytes();
    SKIPPED.iter().any(|s| bytes.eq_ignore_ascii_case(s.as_bytes()))
}

/// The folder name the results sort by: lowercase, used only for ordering.
fn sort_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

#[cfg(test)]
mod tests;
