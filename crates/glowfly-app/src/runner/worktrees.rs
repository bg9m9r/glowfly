//! Checking the folder of a worktree about to be created (`WorktreeEffect::CheckFolder`),
//! removing one, then deleting its branch (and that branch on its remote) when asked
//! (`WorktreeEffect::Remove`), moving one (`WorktreeEffect::Move`), and counting other
//! worktrees' changes in the background (`WorktreeEffect::CountChanges`). A remove or move asked
//! to stops the worktree's file system monitor daemon first, which on Windows holds its folder.

use super::execute::{Cx, Execute};
use crate::effect::WorktreeEffect;
use crate::msg::Msg;
use crate::worktree::{CountRequest, MoveRequest, NewWorktree, RemoteOutcome, RemoveRequest, Removed, WorktreeMsg};
use glowfly_core::branch_ops::delete_with_remote_cmds;
use glowfly_core::gitcli::{self, args};
use glowfly_core::undo::{self, UndoKind};
use glowfly_core::worktree_ops::{self, RemoveOutcome};

impl Execute for WorktreeEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            WorktreeEffect::CheckFolder(r) => check(r),
            WorktreeEffect::Remove(r) => remove(r),
            WorktreeEffect::CountChanges(r) => count(r),
            WorktreeEffect::Move(r) => move_(r),
        })
    }
}

/// Whether git can create the worktree in `request.folder`, as `WorktreeMsg::FolderChecked`.
pub(super) fn check(request: NewWorktree) -> Msg {
    let result = worktree_ops::check_folder(&request.folder).map_err(|p| p.message(&request.folder.display().to_string()));
    Msg::Worktree(WorktreeMsg::FolderChecked { request, result })
}

/// Counts each target's changes, one after another, as `WorktreeMsg::ChangesCounted`. A panic
/// here leaves `RefreshState::counting` set until the tab is switched or loaded afresh, so the
/// tab counts nothing more until then.
pub(super) fn count(r: CountRequest) -> Msg {
    let counts = r
        .targets
        .iter()
        .map(|(path, decision)| glowfly_core::repo::count_worktree(path, decision.as_ref(), glowfly_core::repo::COUNT_TIMEOUT))
        .collect();
    Msg::Worktree(WorktreeMsg::ChangesCounted { refs_key: r.refs_key, counts })
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

/// What a failed command printed, trimmed, or `fallback` when it printed nothing.
fn whole(text: &str, fallback: &str) -> String {
    let text = text.trim();
    if text.is_empty() { fallback } else { text }.to_owned()
}

/// Removes the worktree, then (only once its record is gone) deletes the branch with
/// `git branch -d`, recording that delete for undo. With a remote upstream to delete too, HEAD
/// is first checked to contain the branch (before the local delete takes its ref), and the
/// push that deletes it runs only when that check and the local delete both worked.
pub(super) fn remove(r: RemoveRequest) -> Msg {
    let stopped = stop_first(r.stop_monitor, &r.path);
    let rep = worktree_ops::remove(&r.repo, &r.path, r.force);
    let mut output = joined(stopped, rep.output);
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
                    Some(if pushed { RemoteOutcome::Deleted } else { RemoteOutcome::Failed(whole(&text, "git push failed")) })
                }
                (Some(_), _) => Some(RemoteOutcome::Kept),
                (None, _) => None,
            };
        } else {
            branch = Some(Err(whole(&text, "git branch -d failed")));
        }
    }
    Msg::Worktree(WorktreeMsg::Removed(Box::new(Removed { request: r, outcome: rep.outcome, output, branch, remote, undo })))
}

/// Moves the worktree (`worktree_ops::move_worktree`), as `WorktreeMsg::Moved`.
pub(super) fn move_(request: MoveRequest) -> Msg {
    let stopped = stop_first(request.stop_monitor, &request.from);
    let rep = worktree_ops::move_worktree(&request.repo, &request.from, &request.to);
    Msg::Worktree(WorktreeMsg::Moved { request, outcome: rep.outcome, output: joined(stopped, rep.output) })
}

/// When `stop`, stops the file system monitor daemon watching the worktree at `path`, so it lets
/// go of the folder; its log ("" when none ran or `stop` is false).
fn stop_first(stop: bool, path: &std::path::Path) -> String {
    if stop && path.is_dir() { glowfly_core::repo::stop_monitor(path) } else { String::new() }
}

/// The stop's log, then the remove's or move's.
fn joined(stopped: String, output: String) -> String {
    match (stopped.is_empty(), output.is_empty()) {
        (true, _) => output,
        (false, true) => stopped,
        (false, false) => format!("{stopped}\n{output}"),
    }
}

#[cfg(test)]
#[path = "worktrees_tests.rs"]
mod tests;
