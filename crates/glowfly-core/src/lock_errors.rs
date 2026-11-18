//! libgit2's own lock failures, read from its typed error (`git_errors::classify` reads git's
//! text): the cause, and the lock file's whole path for Delete lock file (`lock_file`).
//!
//! libgit2 says a lock is in the way with code `Locked` in three wordings: "failed to lock file
//! '<path>.lock' for writing", "failed to create locked file '<path>.lock'" and, for every index
//! write, "the index is locked; …" with no path at all, which is then the repository index's
//! own path with `.lock` added. A `Locked` error without a lock file to name (a locked worktree,
//! a file another program holds open) is not a lock file and is never read as one.

use crate::error::{Error, WriteFailed};
use crate::git_errors::Cause;
use git2::{ErrorClass, ErrorCode};
use std::path::Path;

/// `Cause::Locked` when `e` is libgit2 saying a lock file it can name is in the way.
pub fn classify_git2(e: &git2::Error) -> Option<Cause> {
    if e.code() != ErrorCode::Locked {
        return None;
    }
    let file = match quoted_lock(e.message()) {
        Some(path) => file_name(path).to_owned(),
        None if index_locked(e) => "index.lock".to_owned(),
        None => return None,
    };
    Some(Cause::Locked { file })
}

/// libgit2's "the index is locked; …", which names no path.
fn index_locked(e: &git2::Error) -> bool {
    e.class() == ErrorClass::Index && e.message().starts_with("the index is locked")
}

/// The lock file's path quoted in "failed to lock file '<path>' for writing" or "failed to
/// create locked file '<path>'", when it ends in `.lock`. Read up to the last quote, so a path
/// holding a `'` stays whole.
fn quoted_lock(msg: &str) -> Option<&str> {
    let path = if let Some((_, rest)) = msg.split_once("failed to lock file '") {
        rest.rsplit_once("' for writing")?.0
    } else {
        let (_, rest) = msg.split_once("failed to create locked file '")?;
        rest.rsplit_once('\'')?.0
    };
    (path.len() > ".lock".len() && path.ends_with(".lock")).then_some(path)
}

/// The whole path of the lock file libgit2's `e` names, for `lock_file::check`: the quoted one,
/// or for "the index is locked" the index of the repository at `workdir` with `.lock` added.
/// None when `e` isn't a lock file in the way (`classify_git2`) or the path isn't UTF-8.
pub fn lock_path_git2(workdir: &Path, e: &git2::Error) -> Option<String> {
    classify_git2(e)?;
    if let Some(path) = quoted_lock(e.message()) {
        return Some(path.to_owned());
    }
    let repo = git2::Repository::open(workdir).ok()?;
    let index = repo.index().ok()?;
    let lock = match index.path() {
        Some(p) => {
            let mut name = p.file_name()?.to_os_string();
            name.push(".lock");
            p.with_file_name(name)
        }
        None => repo.path().join("index.lock"),
    };
    lock.to_str().map(str::to_owned)
}

/// The lock file `e` says is in the way, from libgit2's error or from git's output (the whole-
/// file ops run git, whose failure is `Error::Other` with its output).
pub fn lock_named(workdir: &Path, e: &Error) -> Option<String> {
    match e {
        Error::Git(g) => lock_path_git2(workdir, g),
        Error::Other(text) => crate::git_errors::lock_path(text),
        _ => None,
    }
}

/// `e` as a write failure: `WriteFailed::Locked` with its own words when a lock file is in the
/// way (`lock_named`), else as `WriteFailed::from` has it.
pub fn locked_write(workdir: &Path, e: Error) -> WriteFailed {
    let Some(lock) = lock_named(workdir, &e) else { return WriteFailed::from(e) };
    let said = match &e {
        Error::Git(g) => g.message().to_owned(),
        Error::Other(text) => text.clone(),
        other => other.to_string(),
    };
    WriteFailed::Locked { lock, said }
}

/// The last component of the lock path `lock`, split on either slash, for the texts.
pub fn file_name(lock: &str) -> &str {
    lock.rsplit(['/', '\\']).next().filter(|f| !f.is_empty()).unwrap_or(lock)
}

#[cfg(test)]
#[path = "lock_errors_tests.rs"]
mod tests;
