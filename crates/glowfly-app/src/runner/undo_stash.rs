//! "Stash changes and undo": local changes go into a stash whose message carries this run's nonce
//! (`undo::undo_stash_message`), the refused undo runs, and the stash is found again only by that
//! message and its commit id. The stash is never dropped except by a `git stash pop --index` that
//! worked; anything else leaves it in the list and the result says so (`UndoStash`).
//!
//! - A merge, pull, cherry-pick, revert or rebase undo (`Action::restores_stash`) stashes staged
//!   and unstaged changes only (untracked files stay, and git refuses rather than write over
//!   one), and puts them back afterwards.
//! - A hard reset, Discard all or stash undo stashes untracked files too and leaves the stash.
//! - When nothing was undone (refused, or not planned), the changes go straight back.
//! - When the stash push itself fails after storing a stash, nothing is undone and that stash
//!   goes back too, or is named.
//! - When the stash list can't be read first, nothing is stashed or undone.

use super::git_ops::{GitRun, UndoGit, undo_run};
use crate::effect::{StashHow, UndoOpts, UndoResult, UndoStash};
use glowfly_core::stash_ops::{self, PutBack};
use glowfly_core::undo::{self, Action};
use std::path::Path;

/// Stashes the changes, undoes (or redoes) `action` and puts the changes back when it should:
/// whether it all worked, the output of every step, and what it means for the history.
pub(super) fn stash_and_undo(cwd: &Path, action: Action, redo: bool, opts: UndoOpts, git: UndoGit) -> (bool, String, UndoResult) {
    let top = match undo::stash_top_at(cwd) {
        Ok(top) => top,
        Err(e) => {
            let output = format!("Could not read the stash list, so nothing was stashed or undone: {e}");
            return (false, output, UndoResult::Failed { redo, stash: None });
        }
    };
    let message = undo::undo_stash_message();
    let restores = action.restores_stash();
    let GitRun { ok: pushed, output: mut text, .. } = git.run(cwd, &[undo::stash_first_command(&message, !restores)]);
    let made = undo::undo_stash_made(cwd, top, &message);
    let note = |how: StashHow, reason: Option<String>| Some(UndoStash { message: message.clone(), how, reason });
    if !pushed {
        // git can store the stash and then fail clearing the tree (a file held open on
        // Windows), leaving it part stashed, part not.
        let Some(stash) = made else { return (false, text, UndoResult::Failed { redo, stash: None }) };
        push_line(&mut text, &format!("git stopped after stashing some of your changes as \"{message}\"; nothing was undone."));
        let how = put_back(cwd, stash, &message, &mut text).unwrap_or(StashHow::Back);
        return (false, text, UndoResult::Failed { redo, stash: note(how, None) });
    }
    let Some(stash) = made else {
        let ran = undo_run(cwd, action, redo, opts, git);
        push_line(&mut text, &ran.output);
        return (ran.ok, text, ran.undo);
    };
    push_line(&mut text, &format!("Your changes were stashed as \"{message}\"."));
    let ran = undo_run(cwd, action, redo, opts, git);
    push_line(&mut text, &ran.output);
    let reason = match &ran.undo {
        UndoResult::Refused { .. } => Some(ran.output.clone()),
        _ => None,
    };
    match ran.undo {
        UndoResult::Done { redo, before, after, .. } if restores => {
            let how = put_back(cwd, stash, &message, &mut text);
            let ok = how.is_none();
            (ok, text, UndoResult::Done { redo, before, after, stash: how.and_then(|h| note(h, None)) })
        }
        UndoResult::Done { redo, before, after, .. } => {
            (ran.ok, text, UndoResult::Done { redo, before, after, stash: note(StashHow::Left, None) })
        }
        // Nothing changed but the stash: the changes go back as they were.
        UndoResult::Refused { redo, stale, offer_stash, .. } if !ran.acted => {
            let how = put_back(cwd, stash, &message, &mut text).unwrap_or(StashHow::Back);
            (false, text, UndoResult::Refused { redo, stale, offer_stash, stash: note(how, reason) })
        }
        UndoResult::Failed { redo, .. } if !ran.acted => {
            let how = put_back(cwd, stash, &message, &mut text).unwrap_or(StashHow::Back);
            (false, text, UndoResult::Failed { redo, stash: note(how, None) })
        }
        // It may have changed something before it stopped: the stash stays, named.
        UndoResult::Refused { redo, stale, offer_stash, .. } => {
            push_line(&mut text, &stays(&message));
            (false, text, UndoResult::Refused { redo, stale, offer_stash, stash: note(StashHow::Left, reason) })
        }
        UndoResult::Failed { redo, .. } => {
            push_line(&mut text, &stays(&message));
            (false, text, UndoResult::Failed { redo, stash: note(StashHow::Left, None) })
        }
        other => {
            push_line(&mut text, &stays(&message));
            (ran.ok, text, other)
        }
    }
}

/// Puts stash `stash` back with `pop --index` only: `None` when it worked, else what became of
/// it, with the line naming the stash that stays.
fn put_back(cwd: &Path, stash: git2::Oid, message: &str, text: &mut String) -> Option<StashHow> {
    let (back, output) = stash_ops::pop_index(cwd, stash);
    push_line(text, &output);
    let how = match back {
        PutBack::Restored => {
            push_line(text, "Your stashed changes are back.");
            return None;
        }
        PutBack::Conflicted { paths } => StashHow::Conflicts(paths.len()),
        PutBack::Gone => {
            push_line(text, &format!("The stash \"{message}\" is no longer in the list, so nothing was put back."));
            return Some(StashHow::Gone);
        }
        PutBack::Failed if output.contains("could not restore untracked files") => StashHow::Partial(not_restored(&output)),
        PutBack::RestoredUnstaged | PutBack::Failed | PutBack::NotReady => StashHow::Failed,
    };
    push_line(text, &stays(message));
    Some(how)
}

/// The untracked files a pop couldn't put back: git's "<path> already exists, no checkout" lines.
fn not_restored(output: &str) -> Vec<String> {
    output.lines().filter_map(|l| l.trim().strip_suffix(" already exists, no checkout")).map(str::to_owned).collect()
}

/// The line saying the stash is still in the list.
fn stays(message: &str) -> String {
    format!("Your changes stay in the stash \"{message}\"; nothing was dropped.")
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

#[cfg(test)]
mod tests {
    #[test]
    fn untracked_files_a_pop_left_out_are_named() {
        let out = "$ git stash pop --index stash@{0}\np.txt already exists, no checkout\nd/q.txt already exists, no checkout\nerror: could not restore untracked files from stash";
        assert_eq!(super::not_restored(out), ["p.txt", "d/q.txt"]);
    }
}
