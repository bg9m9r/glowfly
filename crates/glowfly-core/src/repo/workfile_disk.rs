//! What the working-tree file operations ask the disk about names and folders.
//!
//! Which spellings a folder treats as one name depends on its file system, not on the OS:
//! NTFS, APFS and HFS+ ignore case by default, but a Windows folder or a macOS volume can be
//! case-sensitive, and on Linux FAT, exFAT, CIFS and ext4 casefold folders ignore case. APFS and
//! HFS+ also ignore Unicode normalization (`é` composed or decomposed). So "the same file" is
//! decided by file identity, never by comparing spellings, and how an existing folder is really
//! spelled is read from its parent's listing.
//!
//! The functions that touch the disk are gathered in [`Disk`] so tests can stand in for a file
//! system with other rules (a case-insensitive one on Linux, one without canonicalize).

use crate::error::{Error, Result};
use crate::names::{blocks_here, fold, nfc, path_problem, same_folded};
use git2::Repository;
use std::path::{Component, Path, PathBuf};

/// The disk queries behind the rename and containment checks.
#[derive(Clone, Copy)]
pub(super) struct Disk {
    /// Whether an entry is at the path (a final link is not followed).
    pub exists: fn(&Path) -> bool,
    /// Whether two existing paths are one directory entry, by file identity.
    pub same_file: fn(&Path, &Path) -> bool,
    pub canonicalize: fn(&Path) -> std::io::Result<PathBuf>,
}

/// The real disk.
pub(super) const DISK: Disk = Disk { exists: entry_exists, same_file: same_entry, canonicalize: canonical };

fn entry_exists(p: &Path) -> bool {
    p.symlink_metadata().is_ok()
}

fn canonical(p: &Path) -> std::io::Result<PathBuf> {
    p.canonicalize()
}

/// Whether `a` and `b` are one directory entry: the same file by identity (volume serial and
/// file index on Windows, device and inode on Unix). A final symbolic link is not followed, so
/// a link is never "the same file" as its target.
fn same_entry(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (a.symlink_metadata(), b.symlink_metadata()) else { return false };
    match (ma.file_type().is_symlink(), mb.file_type().is_symlink()) {
        (false, false) => same_file::is_same_file(a, b).unwrap_or(false),
        (true, true) => same_link(a, b, &ma, &mb),
        _ => false,
    }
}

#[cfg(unix)]
fn same_link(_: &Path, _: &Path, ma: &std::fs::Metadata, mb: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    ma.dev() == mb.dev() && ma.ino() == mb.ino()
}

/// Windows can't ask a link's own identity without opening its target here, so two links count
/// as one when they sit in the same folder and point at the same place. The rename that
/// follows still never replaces another entry (`rename_noclobber`).
#[cfg(not(unix))]
fn same_link(a: &Path, b: &Path, _: &std::fs::Metadata, _: &std::fs::Metadata) -> bool {
    let same_dir = match (a.parent(), b.parent()) {
        (Some(pa), Some(pb)) => same_file::is_same_file(pa, pb).unwrap_or(false),
        _ => false,
    };
    same_dir && matches!((a.read_link(), b.read_link()), (Ok(x), Ok(y)) if x == y)
}

/// Whether `dir` lists an entry spelled exactly `name` (byte for byte).
pub(super) fn listed_exactly(dir: &Path, name: &std::ffi::OsStr) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut d| d.any(|e| e.is_ok_and(|e| e.file_name() == name)))
}

/// How the existing entry reached as `name` in `dir` is spelled on disk: `name` itself when
/// listed that way (either Unicode form counts, as APFS and HFS+ may store either), else the
/// listed entry it folds to, else None (reached by another name, such as an 8.3 short name). A
/// folder that can't be listed is taken as spelled as typed.
fn spelled(dir: &Path, name: &str) -> Option<String> {
    let Ok(read) = std::fs::read_dir(dir) else { return Some(name.to_owned()) };
    let typed = nfc(name);
    let mut folded = None;
    for e in read.flatten() {
        let Some(s) = e.file_name().to_str().map(str::to_owned) else { continue };
        if s == name || nfc(&s) == typed {
            return Some(name.to_owned());
        }
        if folded.is_none() && same_folded(&s, name) {
            folded = Some(s);
        }
    }
    folded
}

/// The existing parent folders of `rel` must be spelled as typed. A file system that ignores
/// case would put "Docs/a.md" into an existing "docs", so the file wouldn't be where it was asked
/// to go and git would list it as docs/a.md (a rename to it would change nothing).
pub(super) fn check_parent_spelling(root: &Path, rel: &str, disk: &Disk) -> Result<()> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut dir = root.to_path_buf();
    for i in 0..parts.len() - 1 {
        let next = dir.join(parts[i]);
        if !(disk.exists)(&next) {
            return Ok(());
        }
        let typed = parts[..=i].join("/");
        match spelled(&dir, parts[i]) {
            Some(s) if s == parts[i] => {}
            Some(s) => {
                let actual = parts[..i].iter().copied().chain([s.as_str()]).collect::<Vec<_>>().join("/");
                return Err(Error::Other(format!(
                    "the folder {typed} already exists as {actual}; use that spelling, or rename the folder first"
                )));
            }
            None => return Err(Error::Other(format!("{typed} isn't the folder's own name; type its full name"))),
        }
        dir = next;
    }
    Ok(())
}

/// Guards against a folder swapped for a link between the checks and the write: `dir` must be
/// inside `root` once links are resolved.
pub(super) fn ensure_inside(root: &Path, dir: &Path) -> Result<()> {
    ensure_inside_on(root, dir, &DISK)
}

pub(super) fn ensure_inside_on(root: &Path, dir: &Path, disk: &Disk) -> Result<()> {
    match ((disk.canonicalize)(dir), (disk.canonicalize)(root)) {
        (Ok(d), Ok(r)) if d.starts_with(&r) => Ok(()),
        (Ok(_), Ok(_)) => Err(outside(dir)),
        // Some volumes can't canonicalize (RAM disks, VirtualBox shares, WinFsp and Dokan
        // file systems, some NAS); check by the path's own components instead.
        _ => inside_by_walk(root, dir),
    }
}

fn outside(dir: &Path) -> Error {
    Error::Unsupported(format!("{} is outside the repository", dir.display()))
}

/// `dir` is under `root` by its components (no `..`), and every folder from `root` down to it
/// is a real folder, not a symbolic link or junction, so nothing can lead outside.
fn inside_by_walk(root: &Path, dir: &Path) -> Result<()> {
    let rel = dir.strip_prefix(root).map_err(|_| outside(dir))?;
    let mut at = root.to_path_buf();
    for c in rel.components() {
        let Component::Normal(part) = c else { return Err(outside(dir)) };
        at.push(part);
        let m = at.symlink_metadata()?;
        if m.file_type().is_symlink() {
            return Err(Error::Unsupported(format!(
                "{} is a link, and this volume can't say where it leads; Glowfly won't write through it",
                at.display()
            )));
        }
        if !m.is_dir() {
            return Err(Error::Other(format!("{} is not a folder", at.display())));
        }
    }
    Ok(())
}

/// Warnings for the new path `to` (`from` is a rename's source): a name that won't check out on
/// Windows (where it is refused instead), or a tracked path it shares a name with on a file
/// system that ignores case or normalization.
pub(super) fn new_path_warnings(repo: &Repository, to: &str, from: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if !blocks_here()
        && let Some((part, p)) = path_problem(to)
    {
        out.push(p.warning(part));
    }
    if let Ok(index) = repo.index() {
        let paths = index.iter().filter_map(|e| String::from_utf8(e.path).ok());
        out.extend(tracked_twin(paths, to, from));
    }
    out
}

/// The first tracked path that is `to`, or one of `to`'s folders, under another case or
/// Unicode form; `from` (a rename's source) doesn't count.
pub(super) fn tracked_twin(paths: impl IntoIterator<Item = String>, to: &str, from: Option<&str>) -> Option<String> {
    let twin = |a: &str, b: &str| a != b && if a.is_ascii() && b.is_ascii() { a.eq_ignore_ascii_case(b) } else { fold(a) == fold(b) };
    let to_dirs: Vec<&str> = to.match_indices('/').map(|(i, _)| &to[..i]).collect();
    for p in paths {
        if p == to || Some(p.as_str()) == from {
            continue;
        }
        if twin(&p, to) {
            return Some(format!("{p} is also tracked; Windows and macOS see it and {to} as one file."));
        }
        let dirs = p.match_indices('/').map(|(i, _)| &p[..i]);
        if let Some((d, q)) = to_dirs.iter().zip(dirs).find(|(d, q)| twin(q, d)) {
            return Some(format!("The folder {q} is tracked; Windows and macOS see it and {d} as one folder."));
        }
    }
    None
}
