//! A file's bytes as of a commit, for the file history's File view: finding the commit (or,
//! for a commit that deletes the file, its parent that has it) and the tree entry, the size
//! cap, and a Git LFS pointer's content through git's filters when asked. The text view
//! (`file_at.rs`) and the image preview both read through `read`.

use super::file_at::{BINARY_NOT_SHOWN, FILE_VIEW_MAX_BYTES, FileAtOptions, LfsState};
use crate::error::{Error, Result};
use git2::{ErrorCode, ObjectType, Oid, Repository};
use std::path::Path;

/// How many leading bytes of smudged content are checked for a NUL (as git does).
const BINARY_SNIFF_BYTES: usize = 8000;

/// A file's bytes as of a commit, binary or not.
#[derive(Clone, Debug, PartialEq)]
pub struct FileBytes {
    /// The stored content; a Git LFS file's content when smudged, else its pointer.
    pub bytes: Vec<u8>,
    pub lfs: LfsState,
    /// The commit read: the one asked about, or with `before` the parent that has the file.
    pub read_at: Oid,
}

/// What `read` found, with the repository opened for it.
pub(super) struct Read {
    pub repo: Repository,
    pub bytes: Vec<u8>,
    pub lfs: LfsState,
    pub read_at: Oid,
}

/// The bytes of `path` in commit `oid` (with `o.before`, in its first parent that has it). A
/// folder, a submodule, too large a file, or a path the commit doesn't have is an error that
/// says so; a Git LFS pointer comes back as stored unless `o.smudge` gets its content.
pub fn file_bytes_at(workdir: &Path, oid: Oid, path: &str, o: FileAtOptions) -> Result<FileBytes> {
    let r = read(workdir, oid, path, o, false)?;
    Ok(FileBytes { bytes: r.bytes, lfs: r.lfs, read_at: r.read_at })
}

/// Reads `path` as `file_bytes_at` does. With `text`, binary content is refused before the
/// size is checked (`BINARY_NOT_SHOWN`, with " (stored in Git LFS)" for smudged content).
pub(super) fn read(workdir: &Path, oid: Oid, path: &str, o: FileAtOptions, text: bool) -> Result<Read> {
    let repo = Repository::open(workdir)?;
    let (bytes, lfs, read_at) = read_in(&repo, workdir, oid, path, o, text)?;
    Ok(Read { repo, bytes, lfs, read_at })
}

fn read_in(repo: &Repository, workdir: &Path, oid: Oid, path: &str, o: FileAtOptions, text: bool) -> Result<(Vec<u8>, LfsState, Oid)> {
    let asked = repo.find_commit(oid)?;
    let short = &oid.to_string()[..7];
    let commit = if o.before {
        let has = |p: &git2::Commit<'_>| p.tree().ok().and_then(|t| t.get_path(Path::new(path)).ok()).is_some();
        asked.parents().find(has).ok_or_else(|| Error::NotFound(format!("{path} is not in the parents of {short}")))?
    } else {
        asked
    };
    let read_at = commit.id();
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
    crate::partial::require(repo, workdir, path, [entry.id()])?;
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
    let bytes = match smudged {
        Some(b) => {
            if text && b[..b.len().min(BINARY_SNIFF_BYTES)].contains(&0) {
                return Err(Error::Unsupported(format!("{BINARY_NOT_SHOWN} (stored in Git LFS)")));
            }
            if b.len() > FILE_VIEW_MAX_BYTES {
                return too_large(b.len() as u64);
            }
            b
        }
        None => {
            if text && blob.is_binary() {
                return Err(Error::Unsupported(BINARY_NOT_SHOWN.into()));
            }
            if blob.size() > FILE_VIEW_MAX_BYTES {
                return too_large(blob.size() as u64);
            }
            blob.content().to_vec()
        }
    };
    Ok((bytes, lfs, read_at))
}
