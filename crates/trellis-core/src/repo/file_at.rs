//! A file's text as of a commit, for the file history's File view: the stored blob, split into
//! lines. A Git LFS pointer can be swapped for the file's content (through git's filters), and
//! a commit that deletes the file can be read as the version just before it (both through
//! `file_bytes.rs`, which the image preview reads with too).

use crate::error::Result;
use git2::Oid;
use std::path::Path;

/// The largest file the File view reads.
pub const FILE_VIEW_MAX_BYTES: usize = 8 * 1024 * 1024;
/// How `file_at`'s error for a binary file starts (the File view shows those as an empty state).
pub const BINARY_NOT_SHOWN: &str = "Binary file not shown";

/// What to read for `file_at_with`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileAtOptions {
    /// Read the version in the first parent that has the path (for a commit that deletes it).
    pub before: bool,
    /// Give a Git LFS pointer's real content; only for a repository git may run in unasked.
    pub smudge: bool,
}

/// Whether the text read is a Git LFS file's.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LfsState {
    /// Not a Git LFS pointer.
    #[default]
    None,
    /// A pointer, and these lines are the file's content.
    Content,
    /// A pointer, shown as it is (smudging wasn't asked for).
    Pointer,
    /// A pointer, shown as it is because git-lfs couldn't give the content (why).
    Unavailable(String),
}

/// A file's lines (without their line endings) and its `linguist-language` attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct FileAt {
    pub lines: Vec<String>,
    pub language: Option<String>,
    pub lfs: LfsState,
    /// The commit read: the one asked about, or with `before` the parent that has the file.
    pub read_at: Oid,
}

/// The text of `path` (repo-relative, '/'-separated) in commit `oid`, as stored.
pub fn file_at(workdir: &Path, oid: Oid, path: &str) -> Result<FileAt> {
    file_at_with(workdir, oid, path, FileAtOptions::default())
}

/// The text of `path` in commit `oid` (with `o.before`, in its first parent that has it). Text
/// that isn't UTF-8 is read lossily. A folder, a submodule, a binary or too large a file, or a
/// path the commit doesn't have is an error that says so.
pub fn file_at_with(workdir: &Path, oid: Oid, path: &str, o: FileAtOptions) -> Result<FileAt> {
    let r = super::file_bytes::read(workdir, oid, path, o, true)?;
    // `lines` drops each "\n" or "\r\n" and adds no empty line after a final newline.
    let lines = String::from_utf8_lossy(&r.bytes).lines().map(String::from).collect();
    Ok(FileAt { lines, language: super::attrs::linguist_language(&r.repo, path), lfs: r.lfs, read_at: r.read_at })
}
