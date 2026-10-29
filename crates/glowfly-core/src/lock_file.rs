//! Delete lock file: removing a lock git named in its error ("Unable to create '<path>': File
//! exists"), left behind by a git that crashed or was killed. It is a destructive step, so
//! every rule errs on refusing:
//!
//! - only the exact path git named, which must be absolute and end in `.lock`, with no `..`;
//! - only inside the repository's git dir or common dir, judged after resolving the folder it is
//!   in (a link inside `.git` pointing elsewhere puts it outside);
//! - never through a link: the file itself must still be a plain file (not a link, a folder or
//!   on Windows any reparse point);
//! - only while no git process runs (`git_procs::scan`; Unknown refuses, saying why).
//!
//! `check` answers whether the button may be offered; `delete` checks everything again and
//! removes the file (`persist::remove_file`). A lock that is already gone is fine: the job can
//! simply run again.

use crate::git_procs::{self, GitProcesses};
use crate::paths::{is_within, resolve};
use std::path::{Component, Path, PathBuf};

/// Why a lock file isn't deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The path git named isn't a lock file's (no `.lock` at the end, or not absolute).
    NotLock,
    /// It isn't inside the repository's git dir or common dir.
    Outside,
    /// It is a link, a folder or another special file.
    NotAFile,
    /// These git programs run (`(pid, name)`).
    GitRunning(Vec<(u32, String)>),
    /// Whether git runs couldn't be told.
    Unknown(git_procs::Unknown),
    /// The repository couldn't be opened, or the file read or removed: the error.
    Failed(String),
}

/// What a check or a delete found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// The lock is there, a plain file at this (resolved) path: it may be deleted, or was.
    Stale(PathBuf),
    /// No file is there any more.
    Gone,
}

/// Whether the lock git named as `named` in the repository at `workdir` may be deleted now.
pub fn check(workdir: &Path, named: &str) -> Result<Found, Refusal> {
    check_with(workdir, named, git_procs::scan)
}

/// Checks again (as `check`) and deletes the lock. `Found::Stale` says it was deleted.
pub fn delete(workdir: &Path, named: &str) -> Result<Found, Refusal> {
    delete_with(workdir, named, git_procs::scan)
}

/// `check`, with the process list read by `scan`.
pub(crate) fn check_with(workdir: &Path, named: &str, scan: impl Fn() -> GitProcesses) -> Result<Found, Refusal> {
    let found = locate(workdir, named)?;
    if found == Found::Gone {
        return Ok(found);
    }
    match scan() {
        GitProcesses::None => Ok(found),
        GitProcesses::Running(names) => Err(Refusal::GitRunning(names)),
        GitProcesses::Unknown(why) => Err(Refusal::Unknown(why)),
    }
}

/// `delete`, with the process list read by `scan`.
pub(crate) fn delete_with(workdir: &Path, named: &str, scan: impl Fn() -> GitProcesses) -> Result<Found, Refusal> {
    let found = check_with(workdir, named, scan)?;
    if let Found::Stale(path) = &found {
        crate::persist::remove_file(path).map_err(|e| Refusal::Failed(e.to_string()))?;
    }
    Ok(found)
}

/// Where the lock `named` really is, when it is a plain file inside the repository's git dir or
/// common dir.
fn locate(workdir: &Path, named: &str) -> Result<Found, Refusal> {
    let path = Path::new(named);
    let name = path.file_name().filter(|n| n.to_str().is_some_and(|n| n.len() > ".lock".len() && n.ends_with(".lock")));
    let (Some(name), Some(parent)) = (name, path.parent()) else { return Err(Refusal::NotLock) };
    if !path.is_absolute() {
        return Err(Refusal::NotLock);
    }
    if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Err(Refusal::Outside);
    }
    let repo = git2::Repository::open(workdir).map_err(|e| Refusal::Failed(e.message().to_owned()))?;
    let roots = [resolve(repo.path()), resolve(repo.commondir())];
    let folder = resolve(parent);
    if !roots.iter().any(|r| is_within(&folder, r)) {
        return Err(Refusal::Outside);
    }
    let real = folder.join(name);
    let meta = match std::fs::symlink_metadata(&real) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Found::Gone),
        Err(e) => return Err(Refusal::Failed(e.to_string())),
        Ok(m) => m,
    };
    if !meta.file_type().is_file() || reparse_point(&meta) {
        return Err(Refusal::NotAFile);
    }
    Ok(Found::Stale(real))
}

/// A Windows reparse point (a link, a junction, a cloud placeholder): never deleted.
#[cfg(windows)]
fn reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn reparse_point(_meta: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
#[path = "lock_file_tests.rs"]
mod tests;
