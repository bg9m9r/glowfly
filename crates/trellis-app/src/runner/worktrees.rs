//! Checking the folder of a worktree about to be created (`Effect::CheckWorktreeFolder`),
//! removing one, then deleting its branch (and that branch on its remote) when asked
//! (`Effect::RemoveWorktree`), and moving one (`Effect::MoveWorktree`).

use crate::msg::Msg;
use crate::worktree::{MoveRequest, NewWorktree, RemoteOutcome, RemoveRequest, Removed, WorktreeMsg};
use trellis_core::branch_ops::delete_with_remote_cmds;
use trellis_core::gitcli::{self, args};
use trellis_core::undo::{self, UndoKind};
use trellis_core::worktree_ops::{self, RemoveOutcome};

/// Whether git can create the worktree in `request.folder`, as `WorktreeMsg::FolderChecked`.
pub(super) fn check(request: NewWorktree) -> Msg {
    let result = worktree_ops::check_folder(&request.folder).map_err(|p| p.message(&request.folder.display().to_string()));
    Msg::Worktree(WorktreeMsg::FolderChecked { request, result })
}

/// Runs `cmd` in the request's repository, appending the command line and git's text to
/// `output`. Returns whether it worked and git's text.
fn run_logged(r: &RemoveRequest, cmd: &[String], output: &mut String) -> (bool, String) {
    let (ok, text) = gitcli::run(&r.repo, cmd);
    output.push_str(&format!("\n$ git {}", cmd.join(" ")));
    if !text.trim().is_empty() {
        output.push('\n');
        output.push_str(text.trim_end());
    }
    (ok, text)
}

/// The first line of git's text: the error comes first, hints follow it.
fn first_line<'a>(text: &'a str, fallback: &'a str) -> &'a str {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(fallback)
}

/// Removes the worktree, then (only once its record is gone) deletes the branch with
/// `git branch -d`, recording that delete for undo. With a remote upstream to delete too, HEAD
/// is first checked to contain the branch (before the local delete takes its ref), and the
/// push that deletes it runs only when that check and the local delete both worked.
pub(super) fn remove(r: RemoveRequest) -> Msg {
    let rep = worktree_ops::remove(&r.repo, &r.path, r.force);
    let mut output = rep.output;
    let (mut branch, mut remote, mut undo) = (None, None, None);
    if matches!(rep.outcome, RemoveOutcome::Removed | RemoveOutcome::FolderLeft)
        && let Some(b) = r.delete_branch.clone()
    {
        // [merge-base --is-ancestor, branch -d, push --delete]
        let remote_cmds = r.delete_remote.as_ref().map(|t| delete_with_remote_cmds(&b, t));
        let merged = remote_cmds.as_ref().map(|c| run_logged(&r, &c[0], &mut output).0);
        let before = undo::before(&r.repo, &UndoKind::DeleteBranch(b.clone())).ok();
        let (ok, text) = run_logged(&r, &args(&["branch", "-d", &b]), &mut output);
        if ok {
            branch = Some(Ok(()));
            undo = before.and_then(|x| undo::after(&r.repo, UndoKind::DeleteBranch(b), x).ok().flatten());
            remote = match (remote_cmds, merged) {
                (Some(cmds), Some(true)) => {
                    let (pushed, text) = run_logged(&r, &cmds[2], &mut output);
                    Some(if pushed {
                        RemoteOutcome::Deleted
                    } else {
                        RemoteOutcome::Failed(first_line(&text, "git push failed").to_owned())
                    })
                }
                (Some(_), _) => Some(RemoteOutcome::Kept),
                (None, _) => None,
            };
        } else {
            branch = Some(Err(first_line(&text, "git branch -d failed").to_owned()));
        }
    }
    Msg::Worktree(WorktreeMsg::Removed(Box::new(Removed { request: r, outcome: rep.outcome, output, branch, remote, undo })))
}

/// Moves the worktree (`worktree_ops::move_worktree`), as `WorktreeMsg::Moved`.
pub(super) fn move_(request: MoveRequest) -> Msg {
    let rep = worktree_ops::move_worktree(&request.repo, &request.from, &request.to);
    Msg::Worktree(WorktreeMsg::Moved { request, outcome: rep.outcome, output: rep.output })
}
