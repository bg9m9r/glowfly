//! Checking the folder of a worktree about to be created (`Effect::CheckWorktreeFolder`).

use crate::msg::Msg;
use crate::worktree::{NewWorktree, WorktreeMsg};
use trellis_core::worktree_ops;

/// Whether git can create the worktree in `request.folder`, as `WorktreeMsg::FolderChecked`.
pub(super) fn check(request: NewWorktree) -> Msg {
    let result = worktree_ops::check_folder(&request.folder).map_err(|p| p.message(&request.folder.display().to_string()));
    Msg::Worktree(WorktreeMsg::FolderChecked { request, result })
}
