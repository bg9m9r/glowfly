//! Messages of the WORKTREES section, the Create worktree dialog and the Remove, Lock and Move
//! dialogs (state/worktrees.rs, state/worktree_manage.rs, state/worktree_move.rs,
//! update/worktrees.rs, update/worktree_manage.rs, update/worktree_move.rs, ui/worktrees.rs,
//! ui/worktree_dialog.rs, ui/worktree_manage_dialog.rs, ui/worktree_move_dialog.rs).

use std::path::PathBuf;
use trellis_core::paths::FolderId;
use trellis_core::repo::Tracking;
use trellis_core::undo::Action;
use trellis_core::worktree_ops::{MoveOutcome, RemoveForce, RemoveOutcome, WorktreeBase};

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
    /// From the runner: how the remove went, and the branch delete's result when one was asked
    /// for and the worktree was removed (`undo` when it was deleted and can be restored), then
    /// the remote branch delete's result when that was asked for and the local delete worked.
    Removed {
        request: RemoveRequest,
        outcome: RemoveOutcome,
        output: String,
        branch: Option<Result<(), String>>,
        remote: Option<RemoteOutcome>,
        undo: Option<Action>,
    },
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
        }
    }
}
