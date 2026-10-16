//! Undo and redo: starting them as the single active job (after asking whether to delete a
//! branch that the checkout being undone created), and keeping each tab's history in step with
//! the git jobs that finish in it, offering to stash local changes when they block an undo.

use crate::effect::{Effect, UndoOpts, UndoResult};
use crate::msg::{JobMsg, Msg};
use crate::state::toasts::ToastAction;
use crate::state::undo::{UndoCreatedForm, UndoHistory};
use crate::state::{AppState, CommitDraft, Modal, RunningJob};
use std::time::Instant;
use trellis_core::undo::{Action, Created, HeadPos};

/// The label of the toast button that stashes local changes and runs the refused undo.
pub(super) const STASH_AND_UNDO: &str = "Stash changes and undo";

/// Undoes (or, with `redo`, redoes) the active tab's most recent operation.
pub(super) fn run(st: &mut AppState, redo: bool, now: Instant, fx: &mut Vec<Effect>) {
    let top = if redo { st.undo.redo.last() } else { st.undo.undo.last() }.cloned();
    let Some(action) = top else {
        // Say why when the last operation can't be undone; otherwise there is nothing to do.
        if !redo && st.undo.blocked.is_some() {
            let why = st.undo.undo_tooltip(&st.keymap);
            st.toast(false, why, now);
        }
        return;
    };
    // Undoing a checkout that made a branch asks first whether to delete the branch, while
    // deleting it loses nothing. Another dialog open (the menu's Undo under it) skips the question.
    if !redo
        && let Action::Checkout { from, created: Some(c), .. } = &action
        && unchanged(st, c)
        && st.modal.is_none()
        && !super::ops::busy(st, true)
    {
        let back_to = match from {
            HeadPos::Branch(b) => b.clone(),
            HeadPos::Detached(o) => o.to_string()[..7].to_owned(),
        };
        st.modal = Some(Modal::UndoCreated(UndoCreatedForm::new(action, back_to)));
        return;
    }
    start(st, redo, action, UndoOpts::default(), fx);
}

/// Whether branch `c` still points where the checkout created it, as the snapshot shows.
fn unchanged(st: &AppState, c: &Created) -> bool {
    st.snap.as_ref().is_some_and(|s| s.locals.iter().any(|b| b.name == c.name && b.oid == Some(c.oid)))
}

/// Runs the undo (or redo) of `action` with `opts`, if it is still the top of its stack.
pub(super) fn run_with(st: &mut AppState, redo: bool, action: Action, opts: UndoOpts, fx: &mut Vec<Effect>) {
    if matches!(st.modal, Some(Modal::UndoCreated(_))) {
        st.modal = None;
    }
    let top = if redo { st.undo.redo.last() } else { st.undo.undo.last() };
    if top != Some(&action) {
        st.log(false, "The undo history changed, so nothing was run.");
        return;
    }
    start(st, redo, action, opts, fx);
}

fn start(st: &mut AppState, redo: bool, action: Action, opts: UndoOpts, fx: &mut Vec<Effect>) {
    let label = match &action {
        _ if opts.stash_first => format!("Stash changes and undo {}", action.describe()),
        Action::Checkout { created: Some(c), .. } if opts.delete_created && !redo => {
            format!("Undo {} and delete branch {}", action.describe(), c.name)
        }
        _ => format!("{} {}", if redo { "Redo" } else { "Undo" }, action.describe()),
    };
    super::ops::begin_job(st, &label, true, move |cwd| Effect::UndoRedo { cwd, action, redo, opts }, fx);
}

/// Jobs that never move a local branch, HEAD, index or working tree, so the history stays
/// valid: fetch, push, tag, clone, init, config, adding a remote or changing its URLs, setting
/// or unsetting a branch's upstream,
/// `submodule update` (it moves only the submodule's own checkout; the parent's HEAD, index and
/// files stay put), and the LFS commands that only touch hooks, config or the LFS store (install, fetch, push, prune).
/// A fetch into the repository itself (`fetch . a:b`, a fast-forward without checkout) moves a
/// branch, so it doesn't count; LFS track, untrack, pull and checkout write working-tree files.
pub(super) fn keeps_history(cmds: &[Vec<String>]) -> bool {
    cmds.iter().all(|c| {
        let a: Vec<&str> = c.iter().map(String::as_str).collect();
        match a.as_slice() {
            ["fetch", rest @ ..] => !rest.contains(&"."),
            ["push" | "tag" | "clone" | "init" | "config", ..] => true,
            ["branch", "--unset-upstream", ..] => true,
            // Renaming or removing a remote renames or deletes remote branches that recorded
            // actions can name, so those fall through.
            ["remote", "add" | "set-url", ..] => true,
            ["submodule", "update", ..] => true,
            // A plain add touches no ref of this repository; -b creates a branch.
            ["worktree", "add", rest @ ..] => !rest.contains(&"-b"),
            ["worktree", "remove" | "lock" | "unlock" | "prune", ..] => true,
            ["lfs", "install" | "fetch" | "push" | "prune", ..] => true,
            ["branch", flag, ..] => flag.starts_with("--set-upstream-to"),
            _ => false,
        }
    })
}

/// Updates the history of the tab `job` ran in (active when `here`, otherwise parked).
pub(super) fn finished(st: &mut AppState, job: &RunningJob, ok: bool, result: Option<UndoResult>, here: bool) {
    let offer = match &result {
        Some(UndoResult::Refused { redo, offer_stash: true, .. }) => Some(*redo),
        _ => None,
    };
    let (history, draft): (&mut UndoHistory, &mut CommitDraft) = if here {
        (&mut st.undo, &mut st.draft)
    } else {
        let parked = job.workdir.as_deref().and_then(|w| st.tabs.iter_mut().find(|t| t.is(w))).and_then(|t| t.parked.as_mut());
        let Some(p) = parked else { return }; // closed, or not loaded since
        (&mut p.undo, &mut p.draft)
    };
    match result {
        Some(UndoResult::Recorded(Some(a))) if ok => history.record(a),
        Some(UndoResult::Recorded(_)) => {}
        None => {
            if ok && !job.quiet && !job.keeps_undo {
                history.invalidate(&job.label);
            }
        }
        Some(UndoResult::Done { redo: false, before, .. }) => {
            // An undone commit's message comes back into an empty draft (or one holding only the
            // untouched template), ready to commit again.
            if let Action::Commit { message, amend: false, .. } = &before
                && draft.is_blank()
            {
                draft.fill_from(message);
            }
            history.undone(before);
        }
        Some(UndoResult::Done { redo: true, before, after }) => {
            // The commit exists again, so the message the undo put back is no longer a draft.
            // A draft the user has since edited is kept.
            if let Action::Commit { message, amend: false, .. } = &before {
                let mut restored = CommitDraft::default();
                restored.fill_from(message);
                if draft.message() == restored.message() {
                    draft.summary.clear();
                    draft.description.clear();
                }
            }
            history.redone(&before, *after)
        }
        Some(UndoResult::Refused { redo, stale, offer_stash }) => {
            tracing::debug!(redo, stale, offer_stash, "undo/redo refused");
            if stale {
                history.clear();
            }
        }
        Some(UndoResult::Failed { redo }) => tracing::debug!(redo, "undo/redo failed; the entry is kept"),
    }
    // The refusal's toast (shown just before this) offers to stash the changes in the way and
    // undo anyway, for the active tab only.
    if let (true, Some(redo)) = (here, offer) {
        let top = if redo { st.undo.redo.last() } else { st.undo.undo.last() }.cloned();
        if let Some(top) = top {
            let opts = UndoOpts { stash_first: true, ..Default::default() };
            let msg = Box::new(Msg::Job(JobMsg::UndoWith { redo, action: Box::new(top), opts }));
            st.set_last_toast_action(ToastAction { label: STASH_AND_UNDO.into(), msg, workdir: job.workdir.clone() });
        }
    }
}
