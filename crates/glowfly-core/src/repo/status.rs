//! Reading the working-tree status: (unstaged, staged) file lists, and their counts by kind.

use super::FileEntry;
use crate::error::Result;
use git2::{Repository, RepositoryState, Status, StatusOptions};
use std::collections::HashMap;
use std::path::Path;

/// How many changed paths a working tree has, by kind: each path counted once.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct ChangeCounts {
    /// Modified, renamed, type-changed or conflicted paths.
    pub modified: usize,
    /// New paths, staged or untracked.
    pub added: usize,
    pub deleted: usize,
}

impl ChangeCounts {
    /// The counts of a status read. A path in both lists counts once, by its staged entry.
    pub fn of(staged: &[FileEntry], unstaged: &[FileEntry]) -> Self {
        let mut seen: HashMap<&str, char> = HashMap::new();
        for f in staged.iter().chain(unstaged) {
            seen.entry(&f.path).or_insert(f.status);
        }
        let mut out = Self::default();
        for s in seen.values() {
            match s {
                'A' | '?' => out.added += 1,
                'D' => out.deleted += 1,
                _ => out.modified += 1,
            }
        }
        out
    }

    pub fn total(&self) -> usize {
        self.modified + self.added + self.deleted
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Working-tree status only: (unstaged, staged, repository state). `git_status` false: libgit2
/// only, so no fsmonitor hook or filter runs (repositories not trusted).
pub fn status_at(workdir: &Path, git_status: bool) -> Result<(Vec<FileEntry>, Vec<FileEntry>, RepositoryState)> {
    let repo = crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir)?;
    let (unstaged, staged) = load_status(&repo, git_status)?;
    Ok((unstaged, staged, super::op_state::state(&repo)))
}

/// `status_at`, plus the memo of the index the read started from (`IndexMemo`), read just
/// before the status so a change made during the read shows up in the next comparison. The memo
/// is None past `MAX_INDEX_MEMO` entries or when the index can't be read.
#[allow(clippy::type_complexity)]
pub fn status_and_index(
    workdir: &Path,
    git_status: bool,
) -> Result<(Vec<FileEntry>, Vec<FileEntry>, RepositoryState, Option<super::IndexMemo>)> {
    let repo = crate::repo_pool::lend(workdir, crate::repo_pool::Use::Workdir)?;
    let memo = repo.index().ok().and_then(|i| super::IndexMemo::of(&i));
    let (unstaged, staged) = load_status(&repo, git_status)?;
    Ok((unstaged, staged, super::op_state::state(&repo), memo))
}

/// (unstaged, staged), each sorted by path. Read by git itself when the repository is set up for
/// a fast status (fsmonitor, the untracked cache) or its index is large: libgit2 uses neither.
/// `git_status` false: libgit2 only, so no fsmonitor hook or filter runs (repositories not
/// trusted).
pub(super) fn load_status(repo: &Repository, git_status: bool) -> Result<(Vec<FileEntry>, Vec<FileEntry>)> {
    let found = repo.index().map(|mut i| super::collide::collisions(&mut i)).unwrap_or_default();
    load_status_with(repo, git_status, &found)
}

/// `load_status` with the index's collisions already found (a full load finds them once).
pub(super) fn load_status_with(
    repo: &Repository,
    git_status: bool,
    found: &[super::Collision],
) -> Result<(Vec<FileEntry>, Vec<FileEntry>)> {
    if let Some(workdir) = repo.workdir()
        && git_status
        && wants_git(repo)
        && let Some(lists) = super::status_cli::status(workdir)
    {
        return Ok(mark_rows(repo, lists, found));
    }
    libgit2_status(repo).map(|lists| mark_rows(repo, lists, found))
}

/// Marks a whole read's rows: twins, then LFS pointers.
fn mark_rows(repo: &Repository, lists: (Vec<FileEntry>, Vec<FileEntry>), found: &[super::Collision]) -> (Vec<FileEntry>, Vec<FileEntry>) {
    let (mut unstaged, mut staged) = mark_twins(repo, lists, found);
    super::lfs_marks::mark_wip(repo, &mut unstaged, &mut staged);
    (unstaged, staged)
}

/// Marks the rows whose path shares one file on disk with another index path.
fn mark_twins(
    repo: &Repository,
    (mut unstaged, mut staged): (Vec<FileEntry>, Vec<FileEntry>),
    found: &[super::Collision],
) -> (Vec<FileEntry>, Vec<FileEntry>) {
    use super::collide;
    if !found.is_empty() {
        let (folding, ignorecase) = (collide::folding(repo.path()), collide::ignorecase(repo));
        collide::mark(&mut unstaged, found, folding, ignorecase);
        collide::mark(&mut staged, found, folding, ignorecase);
    }
    (unstaged, staged)
}

/// Entries past which git's own status is faster than libgit2's.
pub(super) const LARGE_INDEX: usize = 50_000;

/// Whether git reads the status faster than libgit2 here.
pub(super) fn wants_git(repo: &Repository) -> bool {
    let Ok(config) = repo.config() else { return false };
    let speedups = super::status_speed::speedups_of(&config);
    speedups.fsmonitor || speedups.untracked_cache || repo.index().is_ok_and(|i| i.len() > LARGE_INDEX)
}

/// (unstaged, staged) read by libgit2 alone: no fsmonitor hook, filter or other program runs.
pub(super) fn libgit2_status(repo: &Repository) -> Result<(Vec<FileEntry>, Vec<FileEntry>)> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true).renames_head_to_index(true).include_ignored(false);
    let (mut unstaged, mut staged) = libgit2_lists(repo, &mut opts)?;
    drop_skipped(repo, &mut unstaged)?;
    unstaged.sort_by(|a, b| a.path.cmp(&b.path));
    staged.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((unstaged, staged))
}

/// The (unstaged, staged) rows libgit2's status gives with `opts`, unsorted: a conflicted path
/// once, as an unstaged '!' row; a staged rename under its new path with the old one in
/// `renamed_from`.
pub(super) fn libgit2_lists(repo: &Repository, opts: &mut StatusOptions) -> Result<(Vec<FileEntry>, Vec<FileEntry>)> {
    let mut unstaged = Vec::new();
    let mut staged = Vec::new();
    // A .gitmodules that can't be parsed fails the status of every gitlink; read it again
    // without submodules rather than failing the whole load.
    let statuses = match repo.statuses(Some(opts)) {
        Ok(s) => s,
        Err(_) => repo.statuses(Some(opts.exclude_submodules(true)))?,
    };
    for e in statuses.iter() {
        let s = e.status();
        // A staged rename is listed under its new path; its old path is kept in renamed_from.
        let bytes = e.head_to_index().and_then(|d| d.new_file().path_bytes()).unwrap_or_else(|| e.path_bytes());
        let row = |status| super::rpath::entry(bytes, status);
        if s.is_conflicted() {
            unstaged.push(row('!'));
            continue;
        }
        if let Some(c) = index_char(s) {
            let mut row = row(c);
            if c == 'R' {
                row.renamed_from = e.head_to_index().and_then(|d| d.old_file().path_bytes()).and_then(super::rpath::text);
            }
            staged.push(row);
        }
        if let Some(c) = worktree_char(s) {
            unstaged.push(row(c));
        }
    }
    Ok((unstaged, staged))
}

/// The staged row's status letter for libgit2's flags (None: nothing staged). A rename whose
/// content changed too carries the modified flag as well; git still calls it a rename, so the
/// rename is checked first.
pub(super) fn index_char(s: Status) -> Option<char> {
    if s.is_index_new() {
        Some('A')
    } else if s.is_index_renamed() {
        Some('R')
    } else if s.is_index_modified() {
        Some('M')
    } else if s.is_index_deleted() {
        Some('D')
    } else if s.is_index_typechange() {
        Some('T')
    } else {
        None
    }
}

/// The unstaged row's status letter for libgit2's flags (None: nothing changed in the work tree).
pub(super) fn worktree_char(s: Status) -> Option<char> {
    if s.is_wt_new() {
        Some('?')
    } else if s.is_wt_modified() {
        Some('M')
    } else if s.is_wt_deleted() {
        Some('D')
    } else if s.is_wt_renamed() {
        Some('R')
    } else if s.is_wt_typechange() {
        Some('T')
    } else {
        None
    }
}

/// Drops unstaged deletions of files a sparse checkout leaves out (skip-worktree): libgit2
/// reports them as deleted, git doesn't.
pub(super) fn drop_skipped(repo: &Repository, unstaged: &mut Vec<FileEntry>) -> Result<()> {
    if !unstaged.iter().any(|f| f.status == 'D') {
        return Ok(());
    }
    let index = repo.index()?;
    let skip = git2::IndexEntryExtendedFlag::SKIP_WORKTREE.bits();
    unstaged.retain(|f| f.status != 'D' || index.get_path(Path::new(&f.path), 0).is_none_or(|e| e.flags_extended & skip == 0));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(path: &str, status: char) -> FileEntry {
        FileEntry::new(path, status)
    }

    #[test]
    fn change_counts_count_each_path_once_by_kind() {
        let staged = [e("new.rs", 'A'), e("gone.rs", 'D'), e("moved.rs", 'R')];
        let unstaged = [e("new.rs", 'M'), e("loose.txt", '?'), e("edit.rs", 'M'), e("kind", 'T'), e("both.rs", '!')];
        let c = ChangeCounts::of(&staged, &unstaged);
        assert_eq!(c, ChangeCounts { modified: 4, added: 2, deleted: 1 });
        assert_eq!((c.total(), c.is_empty()), (7, false));
        assert!(ChangeCounts::of(&[], &[]).is_empty());
        assert_eq!(ChangeCounts::of(&[], &[e("x", 'D')]).deleted, 1);
    }
}
