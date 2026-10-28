//! "Stash changes and undo": local changes (staged, unstaged and untracked) go into a stash
//! named `undo::UNDO_STASH_MESSAGE`, the refused undo runs, and for a merge, pull, cherry-pick,
//! revert or rebase (`Action::restores_stash`) the changes go back with `git stash pop --index`.
//! The stash is never dropped otherwise: after a hard reset, Discard all or stash undo, when the
//! undo fails or is refused, or when git can't put the changes back exactly (with their staged
//! state), it stays in the list and the output says so.

use super::git_ops::{GitRun, UndoGit, undo_run};
use crate::effect::{StashKept, UndoOpts, UndoResult};
use glowfly_core::stash_ops::{self, PutBack};
use glowfly_core::undo::{self, Action, UNDO_STASH_MESSAGE};
use std::path::Path;

/// Stashes the changes, undoes (or redoes) `action` and puts the changes back when it should:
/// whether it all worked, the output of every step, and what it means for the history.
pub(super) fn stash_and_undo(cwd: &Path, action: Action, redo: bool, opts: UndoOpts, git: UndoGit) -> (bool, String, UndoResult) {
    let top = undo::stash_top_at(cwd);
    let GitRun { ok, output, .. } = git.run(cwd, &[undo::stash_first_command()]);
    if !ok {
        return (false, output, UndoResult::Failed { redo });
    }
    let made = undo::undo_stash_made(cwd, top);
    // The list changed, but not with a stash of ours: it is never popped, only named.
    let unknown = made.is_none() && undo::stash_top_at(cwd) != top;
    let mut text = output;
    if made.is_some() {
        text.push_str(&format!("\nYour changes were stashed as \"{UNDO_STASH_MESSAGE}\"."));
    }
    let restores = action.restores_stash();
    let (ok, output, result) = undo_run(cwd, action, redo, opts, git);
    push_line(&mut text, &output);
    let Some(stash) = made else {
        if unknown {
            push_line(&mut text, &stays());
        }
        return (ok, text, result);
    };
    match result {
        UndoResult::Done { redo, before, after, .. } if restores => {
            let (back, output) = stash_ops::pop_index(cwd, stash);
            push_line(&mut text, &output);
            let kept = match back {
                PutBack::Restored => {
                    push_line(&mut text, "Your stashed changes are back.");
                    None
                }
                PutBack::Conflicted { paths } => Some(StashKept::Conflicts(paths.len())),
                PutBack::RestoredUnstaged | PutBack::Failed | PutBack::Gone | PutBack::NotReady => Some(StashKept::Failed),
            };
            if kept.is_some() {
                push_line(&mut text, &stays());
            }
            (kept.is_none(), text, UndoResult::Done { redo, before, after, kept })
        }
        UndoResult::Done { redo, before, after, .. } => (ok, text, UndoResult::Done { redo, before, after, kept: Some(StashKept::Left) }),
        other => {
            push_line(&mut text, &stays());
            (ok, text, other)
        }
    }
}

/// The line saying the stash is still in the list.
fn stays() -> String {
    format!("Your changes stay in the stash \"{UNDO_STASH_MESSAGE}\"; nothing was dropped.")
}

fn push_line(text: &mut String, line: &str) {
    if line.is_empty() {
        return;
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(line);
}
