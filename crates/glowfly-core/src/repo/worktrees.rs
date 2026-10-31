//! The repository's worktrees: the main one and the linked ones registered under the common git
//! dir, read through libgit2 for the left panel, each with the branch it is rebasing or
//! bisecting (`op_state::held_branch`). Other worktrees' uncommitted changes, for their WIP rows
//! in the graph, are counted in `worktree_counts`.

use super::op_state::{BranchUse, HeldBranch, held_branch};
use super::status::ChangeCounts;
use super::worktree_counts::ChangesRead;
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
    /// The branch a rebase or bisect there stopped on, with HEAD detached: git holds it as
    /// checked out there.
    pub held: Option<HeldBranch>,
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
    /// Its uncommitted changes, counted by full loads (or the background count after one) for
    /// worktrees other than the current one. Zero unless `read` is Counted, and for one that
    /// can't be read.
    pub changes: ChangeCounts,
    /// How `changes` was read: NotCounted for the current or a missing one, with the option off,
    /// and from `worktrees_at`.
    pub read: ChangesRead,
}

impl WorktreeInfo {
    pub fn is_main(&self) -> bool {
        self.name.is_none()
    }

    /// How this worktree has local branch `branch` (exact name), if it does: checked out, or
    /// being rebased or bisected there.
    /// Being rebased or bisected comes first: `git bisect start --no-checkout` leaves HEAD on
    /// the branch it bisects, and git refuses to rename it then too.
    pub fn uses(&self, branch: &str) -> Option<BranchUse> {
        let held = self.held.as_ref().filter(|h| h.name == branch).map(|h| h.by);
        held.or_else(|| (self.branch.as_deref() == Some(branch)).then_some(BranchUse::CheckedOut))
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
        let held = held_branch(common.path());
        let current = is_current(&path);
        let folder = crate::paths::FolderId::of(&path);
        out.push(WorktreeInfo {
            name: None,
            path,
            branch,
            head,
            held,
            current,
            locked: None,
            missing: false,
            prunable: false,
            folder,
            changes: ChangeCounts::default(),
            read: ChangesRead::NotCounted,
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
            let git_dir = common.commondir().join("worktrees").join(name);
            let (branch, head) = read_head(&git_dir, &common);
            let held = held_branch(&git_dir);
            let current = is_current(&path);
            let folder = crate::paths::FolderId::of(&path);
            Some(WorktreeInfo {
                name: Some(name.to_owned()),
                path,
                branch,
                head,
                held,
                current,
                locked,
                missing,
                prunable,
                folder,
                changes: ChangeCounts::default(),
                read: ChangesRead::NotCounted,
            })
        })
        .collect();
    linked.sort_by(|a, b| a.path.cmp(&b.path));
    out.extend(linked);
    out
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
        // A branch name git could read as an option shows as a detached HEAD, as in the load.
        return (super::is_safe_ref_name(&branch).then_some(branch), head);
    }
    match Oid::from_str(text) {
        Ok(oid) => (None, Some(oid)),
        Err(_) => (None, None),
    }
}

#[cfg(test)]
#[path = "worktree_wip_tests.rs"]
pub(super) mod wip_tests;
