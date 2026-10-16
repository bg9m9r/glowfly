//! The files an external diff or merge tool opens: copies of the versions it compares, written
//! to a fresh session folder (`temp`), and the working file where the tool should edit or save.

use super::temp::Session;
use crate::error::Result;
use crate::repo::{self, DiffTarget, SideSource};
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};

/// The two files a diff tool compares, and the session holding the copies among them.
#[derive(Debug)]
pub struct DiffFiles {
    /// The older version.
    pub local: PathBuf,
    /// The newer version: for unstaged changes, the working file itself.
    pub remote: PathBuf,
    pub session: Session,
}

/// Writes the versions of `target`'s file for a diff tool to a new session in `root`. A stored
/// version becomes a read-only copy (through git's checkout filters with `filters`, else its
/// stored bytes), a missing one an empty file, and the working tree's version is the working
/// file itself, so edits saved there are kept.
pub fn diff_files(root: &Path, workdir: &Path, target: &DiffTarget, filters: bool) -> Result<DiffFiles> {
    let sides = repo::diff_sides(workdir, target)?;
    let session = Session::create(root)?;
    let written = (|| -> Result<(PathBuf, PathBuf)> {
        let old = side_file(&session, workdir, target.path(), &sides.old, &sides.old_label, filters)?;
        let new = side_file(&session, workdir, target.path(), &sides.new, &sides.new_label, filters)?;
        Ok((old, new))
    })();
    match written {
        Ok((local, remote)) => Ok(DiffFiles { local, remote, session }),
        Err(e) => {
            let _ = session.remove();
            Err(e)
        }
    }
}

fn side_file(session: &Session, workdir: &Path, path: &str, side: &SideSource, label: &str, filters: bool) -> Result<PathBuf> {
    Ok(match side {
        SideSource::Blob { id, path: at } => session.write(label, at, &repo::blob_bytes(workdir, *id, at, filters)?)?,
        SideSource::Empty => session.write("empty", path, &[])?,
        SideSource::Working(p) => workdir.join(p),
    })
}

/// The files a merge tool works with: copies of the three sides, the working file it saves
/// the result to, and the session holding the copies.
#[derive(Debug)]
pub struct MergeFiles {
    /// The common ancestor, or an empty file when both sides added the file.
    pub base: PathBuf,
    pub ours: PathBuf,
    pub theirs: PathBuf,
    /// The working file.
    pub output: PathBuf,
    pub session: Session,
}

/// Writes the stages of the conflict at `path` for a merge tool to a new session in `root`
/// (through git's checkout filters with `filters`, else the stored bytes).
pub fn merge_files(root: &Path, workdir: &Path, path: &str, filters: bool) -> Result<MergeFiles> {
    let sides = repo::merge_sides(workdir, path)?;
    let session = Session::create(root)?;
    let written = (|| -> Result<[PathBuf; 3]> {
        let base = match sides.base {
            Some(id) => repo::blob_bytes(workdir, id, path, filters)?,
            None => Vec::new(),
        };
        let ours = repo::blob_bytes(workdir, sides.ours, path, filters)?;
        let theirs = repo::blob_bytes(workdir, sides.theirs, path, filters)?;
        Ok([session.write("base", path, &base)?, session.write("ours", path, &ours)?, session.write("theirs", path, &theirs)?])
    })();
    match written {
        Ok([base, ours, theirs]) => Ok(MergeFiles { base, ours, theirs, output: workdir.join(path), session }),
        Err(e) => {
            let _ = session.remove();
            Err(e)
        }
    }
}

/// A hash of the file's bytes, to tell whether a tool changed it; None when it can't be read.
pub fn fingerprint(p: &Path) -> Option<u64> {
    let bytes = std::fs::read(p).ok()?;
    let mut h = DefaultHasher::new();
    h.write_usize(bytes.len());
    h.write(&bytes);
    Some(h.finish())
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
