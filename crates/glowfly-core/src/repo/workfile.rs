//! Creating, renaming and deleting one working-tree file, and checking a typed path. A deletion
//! goes to the OS trash unless asked to remove the file permanently (see `removal`).
//!
//! Working tree only: nothing here touches the index or HEAD, so a renamed or deleted tracked
//! file shows up as an unstaged change. The file operations use plain `std::fs`, because no git
//! config, hooks or filters apply to creating, moving or removing a working copy; libgit2 is
//! used only to ask whether a path is ignored and whether it has unresolved conflicts. A rename
//! that is also staged lives in `workfile_mv`, which shares the checks here.
//!
//! Every path is checked before anything is written: it must stay inside the working tree, never
//! enter `.git`, and never go through a symbolic link or junction in its parent folders, so the
//! repository can't be escaped. New names follow `names`: refused on Windows when Windows can't
//! use them, and a warning elsewhere, so the repository stays checkout-able there. Renames live
//! in `workfile_rename`, the disk queries in `workfile_disk`.

use super::removal::{Deleted, Removal, remove_working_copy};
pub(super) use super::workfile_disk::ensure_inside;
use super::workfile_disk::{DISK, check_parent_spelling, new_path_warnings};
use crate::error::{Error, Result};
use crate::names::{WinNameProblem, blocks_here, is_git_dir_name, win_name_problem};
use git2::Repository;
use std::fs::{Metadata, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Why a typed path can't be used for a new file.
#[derive(Debug, Clone, PartialEq)]
pub enum PathProblem {
    Empty,
    Absolute,
    OutsideRepo,
    InsideGitDir,
    /// The path ends with a separator.
    Folder,
    /// Windows can't use this name (only refused on Windows; elsewhere it is a warning).
    BadName(String, WinNameProblem),
}

impl PathProblem {
    pub fn message(&self) -> String {
        match self {
            PathProblem::Empty => "Enter a file name.".into(),
            PathProblem::Absolute => "Enter a path inside the repository.".into(),
            PathProblem::OutsideRepo => "That path leads outside the repository.".into(),
            PathProblem::InsideGitDir => "Files can't be created inside the .git folder.".into(),
            PathProblem::Folder => "Enter a file name, not a folder.".into(),
            PathProblem::BadName(name, why) => format!("{name} can't be used: {}.", why.reason()),
        }
    }
}

/// What was created or renamed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Written {
    /// Git ignores the new path.
    pub ignored: bool,
    /// The new name won't check out on Windows, or is one name with a tracked path where case
    /// or Unicode form is ignored. The file was written all the same.
    pub warnings: Vec<String>,
}

/// Checks a typed repo-relative path for a new file and returns it normalized: '/' separators,
/// no empty or '.' components. Pure; looks at the text only. A name Windows can't use is
/// refused on Windows only (see [`new_path_warning`] elsewhere).
pub fn check_new_path(input: &str) -> Result<String, PathProblem> {
    check_new_path_for(input, blocks_here())
}

/// [`check_new_path`] with Windows' naming rules refused when `windows`.
pub fn check_new_path_for(input: &str, windows: bool) -> Result<String, PathProblem> {
    let text = input.trim().replace('\\', "/");
    let b = text.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/');
    // A leading '/' also covers UNC paths ("//server/share").
    if text.starts_with('/') || drive {
        return Err(PathProblem::Absolute);
    }
    let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() {
        return Err(PathProblem::Empty);
    }
    if parts.contains(&"..") {
        return Err(PathProblem::OutsideRepo);
    }
    if parts.iter().any(|p| is_git_dir_name(p)) {
        return Err(PathProblem::InsideGitDir);
    }
    if text.ends_with('/') {
        return Err(PathProblem::Folder);
    }
    if windows && let Some((bad, why)) = parts.iter().find_map(|p| win_name_problem(p).map(|w| (*p, w))) {
        return Err(PathProblem::BadName(bad.to_owned(), why));
    }
    Ok(parts.join("/"))
}

/// The warning for a checked new path that Windows can't use, where it isn't refused (Linux and
/// macOS): "Windows reserves aux for a device, so aux won't check out on Windows."
pub fn new_path_warning(rel: &str) -> Option<String> {
    new_path_warning_for(rel, blocks_here())
}

pub fn new_path_warning_for(rel: &str, windows: bool) -> Option<String> {
    if windows {
        return None;
    }
    crate::names::path_problem(rel).map(|(part, why)| why.warning(part))
}

/// An existing path to rename or delete, exactly as status lists it ('/' separators). Only the
/// containment rules apply, so a file with an odd name can still be removed, and nothing is
/// trimmed or rewritten: " a.txt" or (outside Windows) "a\b" is that file, never "a.txt" or
/// "a/b". A path the system would read as another file is refused instead.
pub(crate) fn contained(path: &str) -> Result<&str> {
    let parts: Vec<&str> = path.split('/').collect();
    if path.starts_with('/') {
        return Err(problem(PathProblem::Absolute));
    }
    if parts.iter().any(|p| p.is_empty() || *p == ".") {
        return Err(Error::Other(format!("{path:?} isn't a working-tree file path")));
    }
    if parts.contains(&"..") {
        return Err(problem(PathProblem::OutsideRepo));
    }
    if parts.iter().any(|p| is_git_dir_name(p)) {
        return Err(problem(PathProblem::InsideGitDir));
    }
    // Windows reads '\' as a separator, ':' as a drive or stream, and drops trailing dots and
    // spaces, so such a path would reach a different file from the one listed.
    if cfg!(windows) && (path.contains(['\\', ':']) || parts.iter().any(|p| p.ends_with(['.', ' ']))) {
        return Err(Error::Unsupported(format!("{path:?} can't be reached by that name on Windows")));
    }
    Ok(path)
}

pub(super) fn problem(p: PathProblem) -> Error {
    Error::Other(p.message())
}

pub(super) fn open_root(workdir: &Path) -> Result<(Repository, PathBuf)> {
    let repo = Repository::open(workdir)?;
    let root = repo.workdir().ok_or_else(|| Error::Unsupported("a bare repository has no working tree".into()))?.to_path_buf();
    Ok((repo, root))
}

/// The parent folders of `rel` that already exist must be real folders, not files or links.
pub(super) fn check_parents(root: &Path, rel: &str) -> Result<()> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut prefix = root.to_path_buf();
    for i in 0..parts.len() - 1 {
        prefix.push(parts[i]);
        let shown = parts[..=i].join("/");
        match prefix.symlink_metadata() {
            Err(_) => return Ok(()),
            // Junctions count as symbolic links too.
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::Unsupported(format!("{shown} is a symbolic link; Glowfly won't write through it")));
            }
            Ok(m) if !m.is_dir() => return Err(Error::Other(format!("{shown} is a file, so {rel} can't be written"))),
            Ok(_) => {}
        }
    }
    Ok(())
}

/// Creates the missing parent folders of `rel`, returning the ones it made (outermost first).
pub(super) fn create_parents(root: &Path, rel: &str) -> Result<Vec<PathBuf>> {
    let mut created = Vec::new();
    let mut dir = root.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        dir.push(part);
        if dir.symlink_metadata().is_err() {
            if let Err(e) = std::fs::create_dir(&dir) {
                remove_dirs(&created);
                return Err(e.into());
            }
            created.push(dir.clone());
        }
    }
    Ok(created)
}

/// Removes folders `create_parents` made, innermost first, ignoring errors.
pub(super) fn remove_dirs(created: &[PathBuf]) {
    for d in created.iter().rev() {
        let _ = std::fs::remove_dir(d);
    }
}

/// Removes the parent folders of `rel` left empty, stopping at the first that isn't (or at the
/// repository root), as `git rm` does.
pub(super) fn prune_empty_parents(root: &Path, rel: &str) {
    let parts: Vec<&str> = rel.split('/').collect();
    for depth in (1..parts.len()).rev() {
        if std::fs::remove_dir(root.join(parts[..depth].join("/"))).is_err() {
            break;
        }
    }
}

pub(super) fn already_exists(rel: &str, dir: bool) -> Error {
    Error::Other(if dir { format!("a folder named {rel} already exists") } else { format!("{rel} already exists") })
}

pub(super) fn conflicted(repo: &Repository, rel: &str) -> Result<()> {
    let index = repo.index()?;
    if (1..=3).any(|stage| index.get_path(Path::new(rel), stage).is_some()) {
        return Err(Error::Other(format!("{rel} has unresolved conflicts; resolve them first")));
    }
    Ok(())
}

/// The existing file at `rel`: not missing and not a folder (a symbolic link is fine).
pub(super) fn existing(root: &Path, rel: &str) -> Result<Metadata> {
    let m = root.join(rel).symlink_metadata().map_err(|_| Error::NotFound(format!("{rel} is no longer in the working tree")))?;
    if m.is_dir() {
        return Err(Error::Unsupported(format!("{rel} is a folder")));
    }
    Ok(m)
}

/// Creates an empty file at the typed `path`, with any missing folders. Never overwrites
/// anything. Returns whether git ignores the new file, and warnings about its name.
pub fn create_file(workdir: &Path, path: &str) -> Result<Written> {
    let rel = check_new_path(path).map_err(problem)?;
    let (repo, root) = open_root(workdir)?;
    check_parents(&root, &rel)?;
    check_parent_spelling(&root, &rel, &DISK)?;
    let full = root.join(&rel);
    if let Ok(m) = full.symlink_metadata() {
        return Err(already_exists(&rel, m.is_dir()));
    }
    let created = create_parents(&root, &rel)?;
    let written = ensure_inside(&root, full.parent().unwrap_or(&root)).and_then(|()| {
        // create_new also refuses a name that differs only in case on a case-insensitive file
        // system, and a file that appeared since the check above.
        OpenOptions::new().write(true).create_new(true).open(&full).map(drop).map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => already_exists(&rel, full.is_dir()),
            _ => e.into(),
        })
    });
    if let Err(e) = written {
        remove_dirs(&created);
        return Err(e);
    }
    let warnings = new_path_warnings(&repo, &rel, None);
    Ok(Written { ignored: repo.status_should_ignore(Path::new(&rel))?, warnings })
}

/// Deletes the working-tree file (or symbolic link) `path` the way `removal` asks, and the
/// folders it leaves empty. The index is untouched, so a tracked file shows up as an unstaged
/// deletion. When the trash can't take the file, nothing changes (`Error::TrashRefused`).
pub fn delete_file(workdir: &Path, path: &str, removal: Removal) -> Result<Deleted> {
    super::collide::refuse(workdir, path, "delete")?;
    let rel = contained(path)?;
    let (repo, root) = open_root(workdir)?;
    check_parents(&root, rel)?;
    let m = existing(&root, rel)?;
    conflicted(&repo, rel)?;
    // Windows keeps files locked while they are held open; let go before the shell moves one.
    drop(repo);
    match remove_working_copy(&root, rel, &m, removal) {
        Ok(how) => {
            prune_empty_parents(&root, rel);
            Ok(how)
        }
        // The trash reported an error, but the file went: its folders are pruned all the same.
        Err(e @ Error::Other(_)) if root.join(rel).symlink_metadata().is_err() => {
            prune_empty_parents(&root, rel);
            Err(e)
        }
        Err(e) => Err(e),
    }
}
