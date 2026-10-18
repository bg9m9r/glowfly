//! The Remove and Lock worktree dialogs' forms, and the rules and words for removing, locking,
//! unlocking and pruning worktrees, shared by the views and `update` so a disabled menu item and
//! a refused request say the same thing. Pure: paths are compared component by component, and
//! tabs are matched by the identity they were loaded with too (`tabs::Place`).

use super::tabs::{Place, Tab};
use std::path::{Path, PathBuf};
use trellis_core::repo::{Snapshot, Tracking, WorktreeInfo};
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
    /// The branch's upstream on a remote, offered for deletion with it.
    pub remote: Option<RemoteDelete>,
    pub delete_remote: bool,
    /// Another tab shows the worktree: Remove closes it first.
    pub close_tabs: bool,
    /// The acknowledgement that force mode deletes changes for good is ticked.
    pub acked: bool,
    /// The warning shown when the branch is the Git Flow production or development branch.
    pub flow_base: Option<String>,
}

/// The upstream the Remove dialog can delete along with the branch, and why it can't now.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteDelete {
    pub tracking: Tracking,
    /// `branches::delete_with_remote_blocked`: the branch isn't merged into HEAD.
    pub blocked: Option<String>,
}

impl RemoveForm {
    pub fn new(wt: &WorktreeInfo) -> Self {
        Self {
            path: wt.path.clone(),
            name: wt.folder_name(),
            branch: wt.branch.clone(),
            delete_branch: false,
            refusal: wt.locked.clone().map(Refusal::Locked),
            remote: None,
            delete_remote: false,
            close_tabs: false,
            acked: false,
            flow_base: None,
        }
    }

    /// The form with what `snap` and the open tabs (`tabs::open_places`) add: the branch's
    /// remote upstream, whether another tab shows the worktree, and the Git Flow warning.
    pub fn with_snap(mut self, snap: &Snapshot, open_tabs: &[Place]) -> Self {
        self.remote = remote_delete(snap, self.branch.as_deref());
        self.flow_base = super::gitflow_base::worktree_warning(snap, self.branch.as_deref());
        let folder = super::worktrees::find(snap, &self.path).and_then(|w| w.folder);
        self.close_tabs = open_tabs.iter().any(|t| t.is(&self.path, folder));
        self
    }

    /// The remote branch deleted as well: offered, not blocked, and ticked with the branch.
    pub fn remote_ticked(&self) -> Option<&Tracking> {
        let r = self.remote.as_ref().filter(|r| r.blocked.is_none())?;
        (self.delete_branch && self.branch.is_some() && self.delete_remote).then_some(&r.tracking)
    }

    /// The force the Remove button uses.
    pub fn force(&self) -> RemoveForce {
        self.refusal.as_ref().map(|r| r.force()).unwrap_or(RemoveForce::None)
    }

    /// The Remove button's label.
    pub fn button(&self) -> &'static str {
        let ticked = self.delete_branch && self.branch.is_some();
        match (self.refusal.is_some(), ticked, self.remote_ticked().is_some()) {
            (false, false, _) => "Remove",
            (false, true, false) => "Remove and delete branch",
            (false, true, true) => "Remove and delete local and remote branch",
            (true, false, _) => "Force remove",
            (true, true, false) => "Force remove and delete branch",
            (true, true, true) => "Force remove and delete local and remote branch",
        }
    }

    /// Whether the button waits for the acknowledgement: force mode deletes uncommitted
    /// changes for good.
    pub fn needs_ack(&self) -> bool {
        self.refusal.is_some()
    }

    /// The acknowledgement's label, naming what force mode deletes.
    pub fn ack_label(&self) -> Option<String> {
        Some(match self.refusal.as_ref()? {
            Refusal::Changes(n) => format!("Delete {} for good", changed_files(*n)),
            Refusal::Locked(_) => "Delete any uncommitted changes in it for good".to_owned(),
        })
    }

    /// The amber note when another tab shows the worktree.
    pub fn close_note(&self) -> Option<&'static str> {
        self.close_tabs.then_some("It is open in another tab. Remove closes that tab first (and any terminal in it).")
    }

    /// The red note when the remote branch is deleted too.
    pub fn remote_note(&self) -> Option<String> {
        self.remote_ticked().map(|t| format!("{} is deleted on {} for everyone who uses it.", t.merge, t.remote))
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

/// The remote upstream of local branch `branch` that can be deleted with it, and whether HEAD
/// contains the branch (`branches::can_delete_with_remote`, `delete_with_remote_blocked`).
pub fn remote_delete(snap: &Snapshot, branch: Option<&str>) -> Option<RemoteDelete> {
    let b = snap.locals.iter().find(|l| Some(l.name.as_str()) == branch)?;
    let t = super::branches::can_delete_with_remote(b)?;
    let blocked = super::branches::delete_with_remote_blocked(b, t, snap.head_branch.as_deref());
    Some(RemoteDelete { tracking: t.clone(), blocked })
}

/// Why the worktree can't be removed from this tab. Another tab showing it is no reason:
/// Remove closes that tab first.
pub fn remove_blocked(wt: &WorktreeInfo) -> Option<&'static str> {
    manage_blocked(wt)
}

/// The indexes of the tabs showing the worktree `wt`, under any spelling: a tab opened under
/// another name of the folder (a link, a short 8.3 name, another case) is found by its identity.
pub fn showing_tabs(tabs: &[Tab], wt: &WorktreeInfo) -> Vec<usize> {
    tabs.iter().enumerate().filter(|(_, t)| t.path.is_some() && t.place().is(&wt.path, wt.folder)).map(|(i, _)| i).collect()
}

/// The row tooltip's line for a worktree another tab shows (`open_tabs`: `tabs::open_places`).
pub fn open_elsewhere(wt: &WorktreeInfo, open_tabs: &[Place]) -> Option<&'static str> {
    (!wt.current && open_tabs.iter().any(|t| t.is(&wt.path, wt.folder))).then_some("Open in another tab.")
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
pub fn remove_refused(wt: &WorktreeInfo) -> Option<&'static str> {
    if wt.is_main() { Some("The main worktree can't be removed.") } else { remove_blocked(wt) }
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
    format!("{} is no longer a worktree of this repository.", trellis_core::native_path::shown(path))
}

/// `remote` is the remote branch deleted too, as `origin/feature`.
pub fn removed_headline(name: &str, branch: Option<&str>, remote: Option<&str>) -> String {
    match (branch, remote) {
        (Some(b), Some(r)) => format!("Removed worktree {name} and deleted branch {b} and {r}"),
        (Some(b), None) => format!("Removed worktree {name} and deleted branch {b}"),
        (None, _) => format!("Removed worktree {name}"),
    }
}

/// The push that deletes `remote` (as `origin/feature`) failed with `error`.
pub fn remote_failed_text(name: &str, branch: &str, remote: &str, error: &str) -> String {
    format!("Removed worktree {name} and deleted branch {branch}, but {remote} wasn't deleted: {error}")
}

/// `remote` was kept because HEAD doesn't contain `branch`.
pub fn remote_kept_text(name: &str, branch: &str, remote: &str) -> String {
    format!("Removed worktree {name} and deleted branch {branch}, but {remote} was kept: {branch} isn't merged into HEAD.")
}

pub fn folder_left_headline(name: &str, path: &Path) -> String {
    format!(
        "Removed worktree {name} from git, but its folder {} couldn't be fully deleted. Close programs using it, then delete the folder yourself.",
        trellis_core::native_path::shown(path)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn form(refusal: Option<Refusal>) -> RemoveForm {
        RemoveForm {
            path: PathBuf::from("wt"),
            name: "wt".into(),
            branch: None,
            delete_branch: false,
            refusal,
            remote: None,
            delete_remote: false,
            close_tabs: false,
            acked: false,
        }
    }

    #[test]
    fn force_mode_needs_the_acknowledgement() {
        assert!(!form(None).needs_ack());
        assert_eq!(form(None).ack_label(), None);
        let changes = form(Some(Refusal::Changes(3)));
        assert!(changes.needs_ack());
        assert_eq!(changes.ack_label().as_deref(), Some("Delete 3 changed or untracked files for good"));
        assert_eq!(form(Some(Refusal::Changes(0))).ack_label().as_deref(), Some("Delete changed or untracked files for good"));
        let locked = form(Some(Refusal::Locked("usb".into())));
        assert!(locked.needs_ack());
        assert_eq!(locked.ack_label().as_deref(), Some("Delete any uncommitted changes in it for good"));
    }
}
