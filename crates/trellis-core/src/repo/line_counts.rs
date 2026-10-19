//! Per-file lines added and removed for a commit's files and the working tree's staged and
//! unstaged files.
//!
//! libgit2 only (no git process, filter or hook runs), so repositories that aren't trusted get
//! counts too. Each file is keyed by the path its row in `commit_detail` or `status_at` shows.

use super::rpath;
use crate::error::Result;
use git2::{Delta, Diff, DiffOptions, FileMode, Oid, Patch, Repository};
use std::collections::HashMap;
use std::path::Path;

/// The most files of one list counted; the counts of a longer list are marked incomplete.
pub const MAX_COUNTED_FILES: usize = 2_000;

/// Blobs and files larger than this are treated as binary (not counted).
pub const MAX_COUNTED_BYTES: i64 = 4 * 1024 * 1024;

/// Lines added and removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineCount {
    pub added: usize,
    pub removed: usize,
}

impl LineCount {
    /// Lines changed: added plus removed.
    pub fn total(self) -> usize {
        self.added + self.removed
    }

    pub fn add(&mut self, other: LineCount) {
        self.added += other.added;
        self.removed += other.removed;
    }
}

/// The counts of one file list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileLineCounts {
    /// By the row's path. Binary files, files over `MAX_COUNTED_BYTES`, Git LFS pointer changes,
    /// gitlinks and conflicted files have no entry.
    pub files: HashMap<String, LineCount>,
    /// The sum of `files`.
    pub total: LineCount,
    /// The largest `LineCount::total` in `files`: what a file row's bar is scaled to.
    pub largest: usize,
    /// Every file of the list was gone through (false past `MAX_COUNTED_FILES`).
    pub complete: bool,
}

impl FileLineCounts {
    pub fn get(&self, path: &str) -> Option<LineCount> {
        self.files.get(path).copied()
    }
}

/// The counts of the working tree's two lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WipLineCounts {
    /// HEAD to the index, renames under their new path.
    pub staged: FileLineCounts,
    /// The index to the working tree, untracked files counted as added.
    pub unstaged: FileLineCounts,
}

impl WipLineCounts {
    /// Staged plus unstaged, or None unless both lists were counted completely.
    pub fn total(&self) -> Option<LineCount> {
        (self.staged.complete && self.unstaged.complete).then(|| {
            let mut t = self.staged.total;
            t.add(self.unstaged.total);
            t
        })
    }

    /// The largest file change in either list, so the bars of both lists compare.
    pub fn largest(&self) -> usize {
        self.staged.largest.max(self.unstaged.largest)
    }
}

fn opts() -> DiffOptions {
    let mut o = DiffOptions::new();
    o.max_size(MAX_COUNTED_BYTES);
    o
}

/// The counts of commit `oid`'s files against its first parent (every line added for a root
/// commit), from the same diff `commit_detail` lists.
pub fn commit_line_counts(workdir: &Path, oid: Oid) -> Result<FileLineCounts> {
    let repo = Repository::open(workdir)?;
    let mut o = opts();
    let diff = super::diff::commit_diff(&repo, oid, Some(&mut o))?;
    counts_of(&repo, &diff, false, MAX_COUNTED_FILES)
}

/// The counts of the working tree's staged and unstaged files. Ignored files are left out.
pub fn wip_line_counts(workdir: &Path) -> Result<WipLineCounts> {
    let repo = Repository::open(workdir)?;
    let mut o = opts();
    let mut staged = repo.diff_tree_to_index(super::diff::head_tree(&repo).as_ref(), None, Some(&mut o))?;
    staged.find_similar(None)?;
    let mut o = opts();
    o.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true);
    // No rename detection: the status doesn't detect work-tree renames either.
    let unstaged = repo.diff_index_to_workdir(None, Some(&mut o))?;
    Ok(WipLineCounts {
        staged: counts_of(&repo, &staged, false, MAX_COUNTED_FILES)?,
        unstaged: counts_of(&repo, &unstaged, true, MAX_COUNTED_FILES)?,
    })
}

/// The counts of `diff`'s first `cap` deltas; `in_workdir` when its new side is the working tree.
pub(super) fn counts_of(repo: &Repository, diff: &Diff<'_>, in_workdir: bool, cap: usize) -> Result<FileLineCounts> {
    let mut out = FileLineCounts::default();
    for (i, delta) in diff.deltas().enumerate() {
        if i >= cap {
            return Ok(out);
        }
        if matches!(delta.status(), Delta::Conflicted | Delta::Unmodified | Delta::Ignored | Delta::Unreadable)
            || delta.old_file().mode() == FileMode::Commit
            || delta.new_file().mode() == FileMode::Commit
            || super::lfs_marks::delta_is_pointer(repo, &delta, in_workdir)
        {
            continue;
        }
        let Some(patch) = Patch::from_diff(diff, i)? else { continue };
        if patch.delta().flags().is_binary() {
            continue;
        }
        let (_, added, removed) = patch.line_stats()?;
        let count = LineCount { added, removed };
        out.total.add(count);
        out.largest = out.largest.max(count.total());
        out.files.insert(rpath::shown(rpath::delta_bytes(&delta)), count);
    }
    out.complete = true;
    Ok(out)
}

#[cfg(test)]
#[path = "line_counts_tests.rs"]
mod tests;
