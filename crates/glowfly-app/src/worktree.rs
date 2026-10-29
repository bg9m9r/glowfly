//! Messages of the WORKTREES section, the Create worktree dialog and the Remove, Lock and Move
//! dialogs (state/worktrees.rs, state/worktree_manage.rs, state/worktree_move.rs,
//! update/worktrees.rs, update/worktree_manage.rs, update/worktree_move.rs, ui/worktrees.rs,
//! ui/worktree_dialog.rs, ui/worktree_manage_dialog.rs, ui/worktree_move_dialog.rs).

use glowfly_core::paths::FolderId;
use glowfly_core::repo::{Tracking, WorktreeCount};
use glowfly_core::undo::Action;
use glowfly_core::worktree_ops::{MoveOutcome, RemoveForce, RemoveOutcome, WorktreeBase};
use std::path::PathBuf;

/// How a remove went, and the branch delete's result when one was asked for and the worktree
/// was removed (`undo` when it was deleted and can be restored), then the remote branch delete's
/// result when that was asked for and the local delete worked.
#[derive(Clone, Debug, PartialEq)]
pub struct Removed {
    pub request: RemoveRequest,
    pub outcome: RemoveOutcome,
    pub output: String,
    pub branch: Option<Result<(), String>>,
    pub remote: Option<RemoteOutcome>,
    pub undo: Option<Action>,
}

/// A create request, checked by update and then on disk by the runner.
#[derive(Clone, Debug, PartialEq)]
pub struct NewWorktree {
    /// The workdir of the tab the request came from.
    pub repo: PathBuf,
    pub base: WorktreeBase,
    pub folder: PathBuf,
    /// A new branch to create at the base.
    pub branch: Option<String>,
    /// Open the new worktree in a tab once it is created.
    pub open: bool,
}

/// A remove request, checked by update; the runner removes the worktree, then deletes the branch.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveRequest {
    /// The workdir of the tab the request came from.
    pub repo: PathBuf,
    pub path: PathBuf,
    /// The worktree's folder name, for the messages.
    pub name: String,
    pub force: RemoveForce,
    /// A branch to delete with `git branch -d` once the worktree is gone.
    pub delete_branch: Option<String>,
    /// The upstream of `delete_branch` to delete on its remote too, once the local delete
    /// worked and HEAD contains the branch.
    pub delete_remote: Option<Tracking>,
    /// Stop the worktree's file system monitor daemon first (`GitState::stops_monitor`): on
    /// Windows it holds the folder open, and git couldn't delete it.
    pub stop_monitor: bool,
}

/// How the remote branch delete after a remove went.
#[derive(Clone, Debug, PartialEq)]
pub enum RemoteOutcome {
    Deleted,
    /// Not tried: HEAD doesn't contain the branch (`merge-base --is-ancestor` failed).
    Kept,
    /// The push failed; the first line of its output.
    Failed(String),
}

/// Other worktrees' changes the load of `workdir` left Pending, to count in the background.
#[derive(Clone, Debug, PartialEq)]
pub struct CountRequest {
    /// The workdir of the tab the request came from.
    pub workdir: PathBuf,
    /// The snapshot's `refs_key`: a result for another is dropped.
    pub refs_key: u64,
    /// Each worktree's folder, and the trust decision saved for that worktree itself, with its print (not
    /// the tab's): git runs there only as opening it in a tab would allow.
    pub targets: Vec<(PathBuf, Option<glowfly_core::settings::SavedTrust>)>,
}

/// A move request, checked by update; the runner checks the target on disk and moves it.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveRequest {
    /// The workdir of the tab the request came from.
    pub repo: PathBuf,
    pub from: PathBuf,
    pub to: PathBuf,
    /// The worktree's folder name, for the messages.
    pub name: String,
    /// The worktree's identity on disk, which finds the tabs showing it under another spelling.
    pub folder: Option<FolderId>,
    /// Stop the worktree's file system monitor daemon first, as for a remove.
    pub stop_monitor: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorktreeMsg {
    /// Open the worktree at this folder as a repository tab.
    Open(PathBuf),
    /// Open the Create worktree dialog for a local branch, a commit or a remote branch.
    CreateDialog(WorktreeBase),
    /// Open the Create worktree dialog for pull request `id`, whose source branch is `source`.
    CreateForPr { id: u64, source: String },
    /// The dialog's Create worktree: reads the form from `st.modal`.
    Submit,
    /// From the runner: Err(reason) when git would refuse `request.folder`.
    FolderChecked { request: NewWorktree, result: Result<(), String> },
    /// Open the Remove worktree dialog for the worktree at this folder.
    RemoveDialog(PathBuf),
    /// The Remove dialog's button: reads the form from `st.modal`.
    Remove,
    /// From the runner: how the remove went (boxed, as it is much larger than the others).
    Removed(Box<Removed>),
    /// Open the Lock worktree dialog for the worktree at this folder.
    LockDialog(PathBuf),
    /// The Lock dialog's button: reads the form from `st.modal`.
    Lock,
    /// Unlock the worktree at this folder.
    Unlock(PathBuf),
    /// Prune the records of missing, unlocked worktrees.
    Prune,
    /// Open the Move worktree dialog for the worktree at this folder.
    MoveDialog(PathBuf),
    /// The Move dialog's button: reads the form from `st.modal`.
    Move,
    /// From the runner: how the move went.
    Moved { request: MoveRequest, outcome: MoveOutcome, output: String },
    /// From the runner (scoped): the background count of the worktrees a load of the snapshot
    /// with `refs_key` left Pending.
    ChangesCounted { refs_key: u64, counts: Vec<WorktreeCount> },
}

impl WorktreeMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open(..)
            | Self::CreateDialog(..)
            | Self::CreateForPr { .. }
            | Self::Submit
            | Self::FolderChecked { .. }
            | Self::RemoveDialog(..)
            | Self::Remove
            | Self::Removed { .. }
            | Self::LockDialog(..)
            | Self::Lock
            | Self::Unlock(..)
            | Self::Prune
            | Self::MoveDialog(..)
            | Self::Move
            | Self::Moved { .. } => None,
            // The tab counts again when it is loaded.
            Self::ChangesCounted { .. } => None,
        }
    }
}
