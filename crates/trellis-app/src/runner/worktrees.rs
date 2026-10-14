//! Checking the folder of a worktree about to be created (`Effect::CheckWorktreeFolder`), and
//! removing one, then deleting its branch when asked (`Effect::RemoveWorktree`).

use crate::msg::Msg;
use crate::worktree::{NewWorktree, RemoveRequest, WorktreeMsg};
use trellis_core::gitcli::{self, args};
use trellis_core::undo::{self, UndoKind};
use trellis_core::worktree_ops::{self, RemoveOutcome};

/// Whether git can create the worktree in `request.folder`, as `WorktreeMsg::FolderChecked`.
pub(super) fn check(request: NewWorktree) -> Msg {
    let result = worktree_ops::check_folder(&request.folder).map_err(|p| p.message(&request.folder.display().to_string()));
    Msg::Worktree(WorktreeMsg::FolderChecked { request, result })
}

/// Removes the worktree, then (only once its record is gone) deletes the branch with
/// `git branch -d`, recording that delete for undo.
pub(super) fn remove(r: RemoveRequest) -> Msg {
    let rep = worktree_ops::remove(&r.repo, &r.path, r.force);
    let mut output = rep.output;
    let (mut branch, mut undo) = (None, None);
    if matches!(rep.outcome, RemoveOutcome::Removed | RemoveOutcome::FolderLeft)
        && let Some(b) = r.delete_branch.clone()
    {
        let before = undo::before(&r.repo, &UndoKind::DeleteBranch(b.clone())).ok();
        let (ok, text) = gitcli::run(&r.repo, &args(&["branch", "-d", &b]));
        output.push_str(&format!("\n$ git branch -d {b}"));
        if !text.trim().is_empty() {
            output.push('\n');
            output.push_str(text.trim_end());
        }
        if ok {
            branch = Some(Ok(()));
            undo = before.and_then(|x| undo::after(&r.repo, UndoKind::DeleteBranch(b), x).ok().flatten());
        } else {
            // The error comes first; git's hints about -D follow it.
            let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git branch -d failed");
            branch = Some(Err(first.to_owned()));
        }
    }
    Msg::Worktree(WorktreeMsg::Removed { request: r, outcome: rep.outcome, output, branch, undo })
}
