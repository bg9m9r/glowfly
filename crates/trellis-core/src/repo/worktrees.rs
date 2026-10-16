//! The repository's worktrees: the main one and the linked ones registered under the common git
//! dir, read through libgit2 for the left panel, and the other worktrees' uncommitted changes
//! for their WIP rows in the graph.

use super::status::ChangeCounts;
use crate::error::Result;
use git2::{Oid, Repository, WorktreeLockStatus};
use std::path::{Component, Path, PathBuf};

/// One worktree of the repository.
#[derive(Clone, PartialEq, Debug)]
pub struct WorktreeInfo {
    /// Name under <common dir>/worktrees; None for the main worktree.
    pub name: Option<String>,
    /// The worktree folder, rebuilt from its components so it has the platform's separators
    /// (libgit2 reports C:/x/y/ with forward slashes and a trailing slash on Windows).
    pub path: PathBuf,
    /// Checked-out branch (short name); None when detached or HEAD is unreadable.
    pub branch: Option<String>,
    /// HEAD's commit; None for an unborn branch or an unreadable HEAD.
    pub head: Option<Oid>,
    /// The worktree the snapshot was loaded from.
    pub current: bool,
    /// Some(reason) when locked ("" without a reason).
    pub locked: Option<String>,
    /// Its folder (or the gitdir link) is gone: `Worktree::validate()` fails.
    pub missing: bool,
    /// `git worktree prune` would drop it: `Worktree::is_prunable(None)` (missing and not locked).
    pub prunable: bool,
    /// The folder's identity on disk when listed (`paths::FolderId`): what tells whether a tab
    /// shows this worktree under another spelling. None when it is missing.
    pub folder: Option<crate::paths::FolderId>,
    /// Its uncommitted changes, counted by full loads for worktrees other than the current one.
    /// Zero for the current one, a missing one, one that can't be read, or when
    /// `LoadOptions::worktree_wips` is off; `worktrees_at` never counts.
    pub changes: ChangeCounts,
}

impl WorktreeInfo {
    pub fn is_main(&self) -> bool {
        self.name.is_none()
    }

    /// The folder's last path component (the whole path when it has none).
    pub fn folder_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.display().to_string())
    }
}

/// The worktrees of the repository `workdir` belongs to: main first, then linked by path.
pub fn worktrees_at(workdir: &Path) -> Result<Vec<WorktreeInfo>> {
    let repo = Repository::open(workdir)?;
    Ok(list(&repo))
}

/// The worktrees of `repo`'s repository, main first and then linked ones by path. Never fails:
/// an error gives an empty list, and an entry that can't be opened is left out.
pub(super) fn list(repo: &Repository) -> Vec<WorktreeInfo> {
    let Ok(common) = Repository::open(repo.commondir()) else { return Vec::new() };
    let here = repo.workdir().map(normalize);
    let is_current = |path: &Path| here.as_deref().is_some_and(|h| crate::paths::same_folder(path, h));
    let mut out = Vec::new();
    if let Some(workdir) = common.workdir() {
        let path = normalize(workdir);
        let (branch, head) = read_head(common.path(), &common);
        let current = is_current(&path);
        let folder = crate::paths::FolderId::of(&path);
        out.push(WorktreeInfo {
            name: None,
            path,
            branch,
            head,
            current,
            locked: None,
            missing: false,
            prunable: false,
            folder,
            changes: ChangeCounts::default(),
        });
    }
    let Ok(names) = common.worktrees() else { return out };
    let mut linked: Vec<WorktreeInfo> = names
        .iter()
        .flatten()
        .filter_map(|name| {
            let wt = common.find_worktree(name).ok()?;
            let path = normalize(wt.path());
            let locked = match wt.is_locked() {
                Ok(WorktreeLockStatus::Locked(r)) => Some(r.unwrap_or_default().trim_end().to_owned()),
                _ => None,
            };
            let missing = wt.validate().is_err();
            let prunable = wt.is_prunable(None).unwrap_or(false);
            let (branch, head) = read_head(&common.commondir().join("worktrees").join(name), &common);
            let current = is_current(&path);
            let folder = crate::paths::FolderId::of(&path);
            Some(WorktreeInfo {
                name: Some(name.to_owned()),
                path,
                branch,
                head,
                current,
                locked,
                missing,
                prunable,
                folder,
                changes: ChangeCounts::default(),
            })
        })
        .collect();
    linked.sort_by(|a, b| a.path.cmp(&b.path));
    out.extend(linked);
    out
}

/// Counts the uncommitted changes of every listed worktree other than the current one and the
/// missing ones. Read by libgit2 alone (never the git CLI), so no hook, fsmonitor or filter
/// runs: safe while the repository isn't trusted. One that can't be opened or read counts as
/// clean.
pub(super) fn count_changes(list: &mut [WorktreeInfo]) {
    for wt in list.iter_mut().filter(|w| !w.current && !w.missing) {
        let lists = Repository::open(&wt.path).ok().and_then(|repo| super::status::libgit2_status(&repo).ok());
        wt.changes = lists.map(|(unstaged, staged)| ChangeCounts::of(&staged, &unstaged)).unwrap_or_default();
    }
}

/// (index in `list`, HEAD commit) of each worktree that gets a WIP row in the graph: not the
/// current one, not missing, with a HEAD commit and uncommitted changes. In list order.
pub fn worktree_wip_heads(list: &[WorktreeInfo]) -> Vec<(usize, Oid)> {
    list.iter()
        .enumerate()
        .filter(|(_, w)| !w.current && !w.missing && !w.changes.is_empty())
        .filter_map(|(i, w)| Some((i, w.head?)))
        .collect()
}

/// `path` rebuilt from its components: platform separators, no trailing separator.
fn normalize(path: &Path) -> PathBuf {
    path.components().filter(|c| !matches!(c, Component::CurDir)).collect()
}

/// The branch (short name for refs/heads/, the whole ref otherwise) and commit that the HEAD
/// file in `git_dir` names. Reading the file means a worktree whose folder is gone still shows
/// its branch.
fn read_head(git_dir: &Path, common: &Repository) -> (Option<String>, Option<Oid>) {
    let Ok(text) = std::fs::read_to_string(git_dir.join("HEAD")) else { return (None, None) };
    let text = text.trim();
    if let Some(target) = text.strip_prefix("ref:") {
        let full = target.trim();
        let head = common.refname_to_id(full).ok();
        let branch = full.strip_prefix("refs/heads/").unwrap_or(full).to_owned();
        return (Some(branch), head);
    }
    match Oid::from_str(text) {
        Ok(oid) => (None, Some(oid)),
        Err(_) => (None, None),
    }
}

#[cfg(test)]
#[path = "worktree_wip_tests.rs"]
mod wip_tests;
