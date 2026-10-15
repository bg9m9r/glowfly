//! Messages of the WORKTREES section, the Create worktree dialog and the Remove and Lock dialogs
//! (state/worktrees.rs, state/worktree_manage.rs, update/worktrees.rs, update/worktree_manage.rs,
//! ui/worktrees.rs, ui/worktree_dialog.rs, ui/worktree_manage_dialog.rs).

use std::path::PathBuf;
use trellis_core::undo::Action;
use trellis_core::worktree_ops::{RemoveForce, RemoveOutcome, WorktreeBase};

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
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorktreeMsg {
    /// Open the worktree at this folder as a repository tab.
    Open(PathBuf),
    /// Open the Create worktree dialog for a local branch or a commit.
    CreateDialog(WorktreeBase),
    /// The dialog's Create worktree: reads the form from `st.modal`.
    Submit,
    /// From the runner: Err(reason) when git would refuse `request.folder`.
    FolderChecked { request: NewWorktree, result: Result<(), String> },
    /// Open the Remove worktree dialog for the worktree at this folder.
    RemoveDialog(PathBuf),
    /// The Remove dialog's button: reads the form from `st.modal`.
    Remove,
    /// From the runner: how the remove went, and the branch delete's result when one was asked
    /// for and the worktree was removed (`undo` when it was deleted and can be restored).
    Removed { request: RemoveRequest, outcome: RemoveOutcome, output: String, branch: Option<Result<(), String>>, undo: Option<Action> },
    /// Open the Lock worktree dialog for the worktree at this folder.
    LockDialog(PathBuf),
    /// The Lock dialog's button: reads the form from `st.modal`.
    Lock,
    /// Unlock the worktree at this folder.
    Unlock(PathBuf),
    /// Prune the records of missing, unlocked worktrees.
    Prune,
}

impl WorktreeMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open(..)
            | Self::CreateDialog(..)
            | Self::Submit
            | Self::FolderChecked { .. }
            | Self::RemoveDialog(..)
            | Self::Remove
            | Self::Removed { .. }
            | Self::LockDialog(..)
            | Self::Lock
            | Self::Unlock(..)
            | Self::Prune => None,
        }
    }
}
