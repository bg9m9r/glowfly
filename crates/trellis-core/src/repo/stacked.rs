//! A stacked list for a multi-selection that has no combined diff: each selected commit's own
//! changed files, against its first parent, as the single-commit details list them.

use super::FileEntry;
use crate::error::Result;
use git2::{Oid, Repository};
use std::collections::HashSet;
use std::path::Path;

/// One selected commit's own changes (against its first parent), for a stacked list.
#[derive(Clone, PartialEq, Debug)]
pub struct StackedCommit {
    pub oid: Oid,
    /// Its changed files, in libgit2 path order (a root commit's are all 'A').
    pub files: Vec<FileEntry>,
    /// Paths of `files` missing from the working tree when loaded.
    pub absent: HashSet<String>,
}

impl StackedCommit {
    /// Whether `path` was in the working tree when the list was loaded.
    pub fn in_workdir(&self, path: &str) -> bool {
        !self.absent.contains(path)
    }
}

/// Each of `oids`' own changes, in the order given, read with one repository open. A commit
/// that can't be found is an error.
pub fn stacked_changes(workdir: &Path, oids: &[Oid]) -> Result<Vec<StackedCommit>> {
    let repo = Repository::open(workdir)?;
    oids.iter()
        .map(|&oid| {
            let diff = super::diff::commit_diff(&repo, oid, None)?;
            let mut files = super::diff::file_entries(&diff, false);
            super::lfs_marks::mark_delta_files(&repo, &diff, &mut files, false);
            let absent = super::diff::absent_paths(&repo, &files);
            Ok(StackedCommit { oid, files, absent })
        })
        .collect()
}

#[cfg(test)]
#[path = "stacked_tests.rs"]
mod tests;
