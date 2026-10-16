//! Undo and redo: starting them as the single active job, and keeping each tab's history in
//! step with the git jobs that finish in it.

use crate::effect::{Effect, UndoResult};
use crate::state::undo::UndoHistory;
use crate::state::{AppState, CommitDraft, RunningJob};
use std::time::Instant;
use trellis_core::undo::Action;

/// Undoes (or, with `redo`, redoes) the active tab's most recent operation.
pub(super) fn run(st: &mut AppState, redo: bool, now: Instant, fx: &mut Vec<Effect>) {
    let top = if redo { st.undo.redo.last() } else { st.undo.undo.last() }.cloned();
    let Some(action) = top else {
        // Say why when the last operation can't be undone; otherwise there is nothing to do.
        if !redo && st.undo.blocked.is_some() {
            st.toast(false, st.undo.undo_tooltip(), now);
        }
        return;
    };
    let label = format!("{} {}", if redo { "Redo" } else { "Undo" }, action.describe());
    super::ops::begin_job(st, &label, true, move |cwd| Effect::UndoRedo { cwd, action, redo }, fx);
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
            history.redone(&before, after)
        }
        Some(UndoResult::Refused { redo, stale }) => {
            tracing::debug!(redo, stale, "undo/redo refused");
            if stale {
                history.clear();
            }
        }
        Some(UndoResult::Failed { redo }) => tracing::debug!(redo, "undo/redo failed; the entry is kept"),
    }
}
