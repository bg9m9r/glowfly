//! Reading the working-tree status: (unstaged, staged) file lists.

use super::FileEntry;
use crate::error::Result;
use git2::{Repository, RepositoryState, StatusOptions};
use std::path::Path;

/// Working-tree status only: (unstaged, staged, repository state). `git_status` false: libgit2
/// only, so no fsmonitor hook or filter runs (repositories not trusted).
pub fn status_at(workdir: &Path, git_status: bool) -> Result<(Vec<FileEntry>, Vec<FileEntry>, RepositoryState)> {
    let repo = Repository::open(workdir)?;
    let (unstaged, staged) = load_status(&repo, git_status)?;
    Ok((unstaged, staged, repo.state()))
}

/// (unstaged, staged), each sorted by path. Read by git itself when the repository is set up for
/// a fast status (fsmonitor, the untracked cache) or its index is large: libgit2 uses neither.
/// `git_status` false: libgit2 only, so no fsmonitor hook or filter runs (repositories not
/// trusted).
pub(super) fn load_status(repo: &Repository, git_status: bool) -> Result<(Vec<FileEntry>, Vec<FileEntry>)> {
    if let Some(workdir) = repo.workdir()
        && git_status
        && wants_git(repo)
        && let Some(lists) = super::status_cli::status(workdir)
    {
        return Ok(lists);
    }
    libgit2_status(repo)
}

/// Entries past which git's own status is faster than libgit2's.
const LARGE_INDEX: usize = 50_000;

/// Whether git reads the status faster than libgit2 here.
fn wants_git(repo: &Repository) -> bool {
    let Ok(config) = repo.config() else { return false };
    // core.fsmonitor is a boolean (the built-in daemon) or a hook's path.
    let fsmonitor = config.get_bool("core.fsmonitor").unwrap_or_else(|_| config.get_string("core.fsmonitor").is_ok_and(|s| !s.is_empty()));
    fsmonitor || config.get_bool("core.untrackedCache").unwrap_or(false) || repo.index().is_ok_and(|i| i.len() > LARGE_INDEX)
}

pub(super) fn libgit2_status(repo: &Repository) -> Result<(Vec<FileEntry>, Vec<FileEntry>)> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true).renames_head_to_index(true).include_ignored(false);
    let mut unstaged = Vec::new();
    let mut staged = Vec::new();
    // A .gitmodules that can't be parsed fails the status of every gitlink; read it again
    // without submodules rather than failing the whole load.
    let statuses = match repo.statuses(Some(&mut opts)) {
        Ok(s) => s,
        Err(_) => repo.statuses(Some(opts.exclude_submodules(true)))?,
    };
    for e in statuses.iter() {
        let s = e.status();
        let path = e
            .head_to_index()
            .and_then(|d| d.new_file().path().map(|p| p.to_string_lossy().into_owned()))
            .or_else(|| e.path().map(str::to_owned))
            .unwrap_or_default();
        if s.is_conflicted() {
            unstaged.push(FileEntry { path, status: '!' });
            continue;
        }
        let index = if s.is_index_new() {
            Some('A')
        } else if s.is_index_modified() {
            Some('M')
        } else if s.is_index_deleted() {
            Some('D')
        } else if s.is_index_renamed() {
            Some('R')
        } else if s.is_index_typechange() {
            Some('T')
        } else {
            None
        };
        let worktree = if s.is_wt_new() {
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
        };
        if let Some(c) = index {
            staged.push(FileEntry { path: path.clone(), status: c });
        }
        if let Some(c) = worktree {
            unstaged.push(FileEntry { path, status: c });
        }
    }
    unstaged.sort_by(|a, b| a.path.cmp(&b.path));
    staged.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((unstaged, staged))
}
