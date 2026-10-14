//! Messages of the WORKTREES section and the Create worktree dialog (state/worktrees.rs,
//! update/worktrees.rs, ui/worktrees.rs, ui/worktree_dialog.rs).

use std::path::PathBuf;
use trellis_core::worktree_ops::WorktreeBase;

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
}
