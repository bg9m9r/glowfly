//! A file's text as of a commit, for the file history's File view: the raw blob (no filters),
//! split into lines.

use crate::error::{Error, Result};
use git2::{ErrorCode, ObjectType, Oid, Repository};
use std::path::Path;

/// The largest file the File view reads.
pub const FILE_VIEW_MAX_BYTES: usize = 8 * 1024 * 1024;

/// A file's lines (without their line endings) and its `linguist-language` attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct FileAt {
    pub lines: Vec<String>,
    pub language: Option<String>,
}

/// The text of `path` (repo-relative, '/'-separated) in commit `oid`. Text that isn't UTF-8 is
/// read lossily. A folder, a submodule, a binary or too large a file, or a path the commit
/// doesn't have is an error that says so.
pub fn file_at(workdir: &Path, oid: Oid, path: &str) -> Result<FileAt> {
    let repo = Repository::open(workdir)?;
    let tree = repo.find_commit(oid)?.tree()?;
    let entry = match tree.get_path(Path::new(path)) {
        Ok(e) => e,
        Err(e) if e.code() == ErrorCode::NotFound => {
            return Err(Error::NotFound(format!("{path} is not in commit {}", &oid.to_string()[..7])));
        }
        Err(e) => return Err(e.into()),
    };
    match entry.kind() {
        Some(ObjectType::Tree) => return Err(Error::Unsupported(format!("{path} is a folder"))),
        Some(ObjectType::Commit) => return Err(Error::Unsupported(format!("{path} is a submodule"))),
        _ => {}
    }
    let blob = repo.find_blob(entry.id())?;
    if blob.is_binary() {
        return Err(Error::Unsupported("Binary file not shown".into()));
    }
    if blob.size() > FILE_VIEW_MAX_BYTES {
        let mb = blob.size().div_ceil(1024 * 1024);
        return Err(Error::Unsupported(format!("{path} is too large to show ({mb} MB)")));
    }
    // `lines` drops each "\n" or "\r\n" and adds no empty line after a final newline.
    let lines = String::from_utf8_lossy(blob.content()).lines().map(String::from).collect();
    Ok(FileAt { lines, language: super::attrs::linguist_language(&repo, path) })
}
