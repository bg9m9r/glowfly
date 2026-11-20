//! Per-file lines added and removed for a commit's files and the working tree's staged and
//! unstaged files.
//!
//! libgit2 only (no git process, filter or hook runs), so repositories that aren't trusted get
//! counts too. Each file is keyed by the path its row in `commit_detail` or `status_at` shows.

use super::attr_source::{AttrSource, attr_repo};
use super::rpath;
pub use super::wip_paths::WipPaths;
use crate::error::Result;
use crate::repo_pool;
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
    let repo = repo_pool::lend(workdir, repo_pool::Use::Workdir)?;
    // Binary files (not counted) as the commit's own gitattributes say.
    let paths = super::diff::commit_paths(&repo, oid);
    let attrs = attr_repo(&repo, AttrSource::Commit(oid), &paths.iter().map(String::as_str).collect::<Vec<_>>());
    let mut o = opts();
    let diff = super::diff::commit_diff(&attrs, oid, Some(&mut o))?;
    counts_of(&attrs, &diff, false, MAX_COUNTED_FILES)
}

/// The counts of the working tree's staged and unstaged files, read over the whole tree. Ignored
/// files are left out.
pub fn wip_line_counts(workdir: &Path) -> Result<WipLineCounts> {
    wip_counts(&*repo_pool::lend(workdir, repo_pool::Use::Workdir)?, None)
}

/// `wip_line_counts` for the rows of one status read (`WipPaths`): only those paths are diffed,
/// each taken literally, so the time follows the changed files and not the size of the tree.
/// Rows past the paths' cap have no counts, and their list is incomplete.
pub fn wip_line_counts_of(workdir: &Path, paths: &WipPaths) -> Result<WipLineCounts> {
    wip_counts(&*repo_pool::lend(workdir, repo_pool::Use::Workdir)?, Some(paths))
}

/// `o` limited to `paths`, each matched exactly or as a folder: no pattern.
fn scoped<'o>(o: &'o mut DiffOptions, paths: &WipPaths) -> &'o mut DiffOptions {
    for p in &paths.paths {
        o.pathspec(p.as_slice());
    }
    o.disable_pathspec_match(true)
}

fn wip_counts(repo: &Repository, scope: Option<&WipPaths>) -> Result<WipLineCounts> {
    crate::tree_files::check(repo)?;
    // An empty path list would mean every path.
    if scope.is_some_and(WipPaths::is_empty) {
        return Ok(WipLineCounts {
            staged: FileLineCounts { complete: true, ..Default::default() },
            unstaged: FileLineCounts { complete: true, ..Default::default() },
        });
    }
    // Staged files by the index's gitattributes, unstaged ones by the working tree's.
    let mut staged = {
        let _s = tracing::debug_span!("line_counts.staged", paths = scope.map(|p| p.paths.len())).entered();
        let index = attr_repo(repo, AttrSource::Index, &[]);
        let mut o = opts();
        if let Some(p) = scope.filter(|p| !p.staged_whole) {
            scoped(&mut o, p);
        }
        let mut diff = index.diff_tree_to_index(super::diff::head_tree(&index).as_ref(), None, Some(&mut o))?;
        diff.find_similar(None)?;
        let _c = tracing::debug_span!("line_counts.count", list = "staged", deltas = diff.deltas().len()).entered();
        counts_of(&index, &diff, false, MAX_COUNTED_FILES)?
    };
    let mut unstaged = {
        let _s = tracing::debug_span!("line_counts.unstaged", paths = scope.map(|p| p.paths.len())).entered();
        let mut o = opts();
        o.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true);
        if let Some(p) = scope {
            scoped(&mut o, p);
        }
        // No rename detection: the status doesn't detect work-tree renames either.
        let diff = repo.diff_index_to_workdir(None, Some(crate::git_file::skip_offsite_submodules(repo, &mut o)))?;
        let _c = tracing::debug_span!("line_counts.count", list = "unstaged", deltas = diff.deltas().len()).entered();
        counts_of(repo, &diff, true, MAX_COUNTED_FILES)?
    };
    if let Some(p) = scope {
        staged.complete &= !p.staged_more;
        unstaged.complete &= !p.unstaged_more;
    }
    Ok(WipLineCounts { staged, unstaged })
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
#[cfg(test)]
#[path = "line_counts_wip_tests.rs"]
mod wip_tests;
