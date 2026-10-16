//! The Remove and Lock worktree dialogs' forms, and the rules and words for removing, locking,
//! unlocking and pruning worktrees, shared by the views and `update` so a disabled menu item and
//! a refused request say the same thing. Pure: paths are compared component by component.

use super::tabs::Place;
use std::path::{Path, PathBuf};
use trellis_core::repo::{Snapshot, WorktreeInfo};
use trellis_core::worktree_ops::{Refusal, RemoveForce};

/// What the Remove worktree dialog is showing.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveForm {
    pub path: PathBuf,
    pub name: String,
    /// The branch checked out in the worktree, offered for deletion.
    pub branch: Option<String>,
    pub delete_branch: bool,
    /// Why removing needs force: known up front for a lock, or reported by a first try. Some
    /// puts the dialog in force mode.
    pub refusal: Option<Refusal>,
}

impl RemoveForm {
    pub fn new(wt: &WorktreeInfo) -> Self {
        Self {
            path: wt.path.clone(),
            name: wt.folder_name(),
            branch: wt.branch.clone(),
            delete_branch: false,
            refusal: wt.locked.clone().map(Refusal::Locked),
        }
    }

    /// The force the Remove button uses.
    pub fn force(&self) -> RemoveForce {
        self.refusal.as_ref().map(|r| r.force()).unwrap_or(RemoveForce::None)
    }

    /// The Remove button's label.
    pub fn button(&self) -> &'static str {
        let ticked = self.delete_branch && self.branch.is_some();
        match (self.refusal.is_some(), ticked) {
            (false, false) => "Remove",
            (false, true) => "Remove and delete branch",
            (true, false) => "Force remove",
            (true, true) => "Force remove and delete branch",
        }
    }

    /// Why force is needed, in force mode.
    pub fn force_note(&self) -> Option<String> {
        Some(match self.refusal.as_ref()? {
            Refusal::Locked(r) => {
                let first =
                    if r.trim().is_empty() { "This worktree is locked.".to_owned() } else { format!("This worktree is locked ({r}).") };
                format!("{first} Force remove ignores the lock and also deletes any uncommitted changes in it.")
            }
            Refusal::Changes(n) => format!("It has {}. Force remove deletes them for good.", changed_files(*n)),
        })
    }
}

/// What the Lock worktree dialog is editing.
#[derive(Clone, Debug, PartialEq)]
pub struct LockForm {
    pub path: PathBuf,
    pub name: String,
    pub reason: String,
}

impl LockForm {
    pub fn new(wt: &WorktreeInfo) -> Self {
        Self { path: wt.path.clone(), name: wt.folder_name(), reason: String::new() }
    }
}

fn changed_files(n: usize) -> String {
    if n == 0 { "changed or untracked files".to_owned() } else { format!("{n} changed or untracked files") }
}

/// Why the worktree can't be locked, unlocked or removed from this tab. The main worktree's
/// row never offers these.
pub fn manage_blocked(wt: &WorktreeInfo) -> Option<&'static str> {
    wt.current.then_some("This worktree is open in this tab.")
}

/// Why the worktree can't be removed: also when another tab shows it, since its folder would
/// disappear under that tab.
/// `open_tabs` are the tabs' places (`tabs::open_places`): a tab opened under another spelling
/// of the folder (a link, a short 8.3 name, another case) is found by its identity.
pub fn remove_blocked(wt: &WorktreeInfo, open_tabs: &[Place]) -> Option<&'static str> {
    manage_blocked(wt)
        .or_else(|| open_tabs.iter().any(|t| t.is(&wt.path, wt.folder)).then_some("It is open in another tab. Close that tab first."))
}

/// Why a missing row's Prune would leave it.
pub fn row_prune_blocked(wt: &WorktreeInfo) -> Option<&'static str> {
    wt.locked.is_some().then_some("It is locked, so prune keeps it. Unlock it first.")
}

/// Why the section's Prune would do nothing.
pub fn prune_blocked(snap: &Snapshot) -> Option<&'static str> {
    (!snap.worktrees.iter().any(|w| w.prunable)).then_some("No worktree records to prune: every folder is present, or locked.")
}

/// Why `update` refuses to remove the worktree.
pub fn remove_refused(wt: &WorktreeInfo, open_tabs: &[Place]) -> Option<&'static str> {
    if wt.is_main() { Some("The main worktree can't be removed.") } else { remove_blocked(wt, open_tabs) }
}

/// Why `update` refuses to lock the worktree.
pub fn lock_refused(wt: &WorktreeInfo) -> Option<&'static str> {
    if wt.is_main() {
        Some("The main worktree can't be locked.")
    } else if wt.locked.is_some() {
        Some("It is already locked.")
    } else {
        manage_blocked(wt)
    }
}

/// Why `update` refuses to unlock the worktree.
pub fn unlock_refused(wt: &WorktreeInfo) -> Option<&'static str> {
    if wt.is_main() {
        Some("The main worktree can't be unlocked.")
    } else if wt.locked.is_none() {
        Some("It isn't locked.")
    } else {
        manage_blocked(wt)
    }
}

/// Logged when a dialog's or menu's worktree is no longer listed.
pub fn gone_text(path: &Path) -> String {
    format!("{} is no longer a worktree of this repository.", path.display())
}

pub fn removed_headline(name: &str, branch: Option<&str>) -> String {
    match branch {
        Some(b) => format!("Removed worktree {name} and deleted branch {b}"),
        None => format!("Removed worktree {name}"),
    }
}

pub fn folder_left_headline(name: &str, path: &Path) -> String {
    format!(
        "Removed worktree {name} from git, but its folder {} couldn't be fully deleted. Close programs using it, then delete the folder yourself.",
        path.display()
    )
}

pub fn branch_failed_text(name: &str, branch: &str, error: &str) -> String {
    format!("Removed worktree {name}, but branch {branch} wasn't deleted: {error}")
}

pub fn needs_force_text(name: &str, refusal: &Refusal) -> String {
    match refusal {
        Refusal::Changes(n) => format!("{name} wasn't removed: it has {}", changed_files(*n)),
        Refusal::Locked(_) => format!("{name} wasn't removed: it is locked"),
    }
}
