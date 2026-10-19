//! A file's text as of a commit, for the file history's File view: the stored blob, split into
//! lines. A Git LFS pointer can be swapped for the file's content (through git's filters), and
//! a commit that deletes the file can be read as the version just before it.

use crate::error::{Error, Result};
use git2::{ErrorCode, ObjectType, Oid, Repository};
use std::path::Path;

/// The largest file the File view reads.
pub const FILE_VIEW_MAX_BYTES: usize = 8 * 1024 * 1024;
/// How `file_at`'s error for a binary file starts (the File view shows those as an empty state).
pub const BINARY_NOT_SHOWN: &str = "Binary file not shown";

/// How many leading bytes of smudged content are checked for a NUL (as git does).
const BINARY_SNIFF_BYTES: usize = 8000;

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
    let repo = Repository::open(workdir)?;
    let asked = repo.find_commit(oid)?;
    let short = &oid.to_string()[..7];
    let commit = if o.before {
        let has = |p: &git2::Commit<'_>| p.tree().ok().and_then(|t| t.get_path(Path::new(path)).ok()).is_some();
        asked.parents().find(has).ok_or_else(|| Error::NotFound(format!("{path} is not in the parents of {short}")))?
    } else {
        asked
    };
    let entry = match commit.tree()?.get_path(Path::new(path)) {
        Ok(e) => e,
        Err(e) if e.code() == ErrorCode::NotFound => return Err(Error::NotFound(format!("{path} is not in commit {short}"))),
        Err(e) => return Err(e.into()),
    };
    match entry.kind() {
        Some(ObjectType::Tree) => return Err(Error::Unsupported(format!("{path} is a folder"))),
        Some(ObjectType::Commit) => return Err(Error::Unsupported(format!("{path} is a submodule"))),
        _ => {}
    }
    let blob = repo.find_blob(entry.id())?;
    let too_large = |size: u64| {
        let mb = size.div_ceil(1024 * 1024);
        Err(Error::Unsupported(format!("{path} is too large to show ({mb} MB)")))
    };
    let (lfs, smudged) = match crate::lfs::pointer::parse(blob.content()) {
        None => (LfsState::None, None),
        Some(_) if !o.smudge => (LfsState::Pointer, None),
        Some(p) if p.size > FILE_VIEW_MAX_BYTES as u64 => return too_large(p.size),
        Some(_) => match super::tool_sides::blob_bytes(workdir, entry.id(), path, true) {
            Ok(b) if crate::lfs::pointer::parse(&b).is_some() => {
                (LfsState::Unavailable("Git LFS isn't set up here, so git gave back the pointer".into()), None)
            }
            Ok(b) => (LfsState::Content, Some(b)),
            Err(e) => (LfsState::Unavailable(e.to_string()), None),
        },
    };
    let bytes = match &smudged {
        Some(b) => {
            if b[..b.len().min(BINARY_SNIFF_BYTES)].contains(&0) {
                return Err(Error::Unsupported(format!("{BINARY_NOT_SHOWN} (stored in Git LFS)")));
            }
            b.as_slice()
        }
        None if blob.is_binary() => return Err(Error::Unsupported(BINARY_NOT_SHOWN.into())),
        None => blob.content(),
    };
    if bytes.len() > FILE_VIEW_MAX_BYTES {
        return too_large(bytes.len() as u64);
    }
    // `lines` drops each "\n" or "\r\n" and adds no empty line after a final newline.
    let lines = String::from_utf8_lossy(bytes).lines().map(String::from).collect();
    Ok(FileAt { lines, language: super::attrs::linguist_language(&repo, path), lfs, read_at: commit.id() })
}
