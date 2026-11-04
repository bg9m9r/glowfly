//! The files git tracks: the paths in the index, for finding a file by name.

use crate::error::Result;
use git2::Repository;
use std::path::Path;

/// A gitlink (submodule) entry's mode in the index.
const GITLINK: u32 = 0o160000;

/// Why [`tracked_paths`] couldn't list the files: libgit2's (or the system's) own words. The
/// model words the rest (`state/error_words/typed.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct ListFilesFailed {
    pub said: String,
}

/// The paths in the index ('/'-separated, in byte order, each once): the files git tracks
/// now, including staged additions. Conflicted paths appear once. Submodules (gitlinks) and
/// paths that aren't valid UTF-8 are left out.
pub fn tracked_paths(workdir: &Path) -> std::result::Result<Vec<String>, ListFilesFailed> {
    read_index(workdir).map_err(|e| ListFilesFailed { said: e.to_string() })
}

/// [`tracked_paths`], with the error as it came.
fn read_index(workdir: &Path) -> Result<Vec<String>> {
    let repo = Repository::open(workdir)?;
    // A fresh read of the index file, not a copy cached by an earlier load.
    let index = repo.index()?;
    let mut out = Vec::with_capacity(index.len());
    for entry in index.iter() {
        if entry.mode == GITLINK {
            continue;
        }
        if let Ok(path) = String::from_utf8(entry.path) {
            out.push(path);
        }
    }
    // The index is in byte order, except where the file system ignores case (libgit2 then
    // sorts ignoring case): sort by bytes so the order is the same everywhere. A conflict's
    // stages share a path and end up next to each other.
    out.sort_unstable();
    out.dedup();
    Ok(out)
}
