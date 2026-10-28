//! Re-reading the status after an index change made outside Glowfly (a `git add`, `reset`,
//! `rm --cached`, `mv` or `update-index` in a terminal, an IDE staging files): the index is read
//! and compared with the memo of the last read (`IndexMemo::changed`), and only the changed
//! paths' rows of both lists are read again, with the work-tree paths the same burst named.
//!
//! Rename detection sees only the paths read, so the staged rows that may pair up with a changed
//! path (every staged addition, deletion and rename, with each rename's old path) are read too:
//! a deletion staged next to an earlier staged addition becomes one rename row, and a reset
//! rename splits again.
//!
//! `Ok(None)` asks for a whole read: an index too large for a memo, a changed entry that is
//! conflicted (stage 1-3, before or after) or whose path isn't UTF-8, or a read set that can't
//! be read on its own (as for `status_of_paths`: too many paths, an ignore or attributes file,
//! a git command line too long). A rewrite that only refreshes stat data reads nothing.

use super::status_paths::{PathsStatus, read_paths};
use super::{FileEntry, IndexMemo};
use crate::error::Result;
use git2::Repository;
use std::path::Path;

/// What an index read found.
#[derive(Debug)]
pub struct IndexStatus {
    /// The memo of the index this read compared against the old one.
    pub memo: IndexMemo,
    /// The paths read and their unstaged rows (see `merge_unstaged`).
    pub paths: PathsStatus,
    /// The staged rows under `paths.covered` (see `merge_staged`).
    pub staged: Vec<FileEntry>,
}

/// Reads the rows the index change since `old` can have changed (see the module doc).
/// `partners`: the paths of the staged additions, deletions and renames on screen, with the
/// renames' old paths; `watched`: the work-tree paths the watcher named with the change.
pub fn status_after_index_change(
    workdir: &Path,
    old: &IndexMemo,
    partners: &[String],
    watched: &[String],
    git_status: bool,
) -> Result<Option<IndexStatus>> {
    let repo = Repository::open(workdir)?;
    let Some(memo) = IndexMemo::of(&repo.index()?) else { return Ok(None) };
    let delta = old.changed(&memo);
    if delta.conflicted || delta.not_utf8 {
        return Ok(None);
    }
    let mut want: Vec<String> = delta.paths.into_iter().chain(watched.iter().cloned()).collect();
    if want.is_empty() {
        let paths =
            PathsStatus { covered: Vec::new(), unstaged: Vec::new(), state: super::op_state::state(&repo), submodule_touched: false };
        return Ok(Some(IndexStatus { memo, paths, staged: Vec::new() }));
    }
    // Any path read may be one half of a staged rename.
    want.extend(partners.iter().cloned());
    want.sort();
    want.dedup();
    Ok(read_paths(workdir, &want, git_status, true)?.map(|(paths, staged)| IndexStatus { memo, paths, staged }))
}

#[cfg(test)]
#[path = "status_index_tests.rs"]
mod tests;
