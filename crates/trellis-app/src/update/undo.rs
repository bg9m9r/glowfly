//! Undo and redo: starting them as the single active job, and keeping each tab's history in
//! step with the git jobs that finish in it.

use crate::effect::{Effect, UndoResult};
use crate::msg::Msg;
use crate::state::undo::UndoHistory;
use crate::state::{AppState, CommitDraft, RunningJob};
use std::time::Instant;
use trellis_core::undo::Action;

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    let redo = matches!(msg, Msg::Redo);
    let top = if redo { st.undo.redo.last() } else { st.undo.undo.last() }.cloned();
    let Some(action) = top else {
        // Say why when the last operation can't be undone; otherwise there is nothing to do.
        if !redo && st.undo.blocked.is_some() {
            st.toast(false, st.undo.undo_tooltip(), now);
        }
        return;
    };
    if super::ops::busy(st, false) {
        return;
    }
    let Some(cwd) = st.workdir() else { return };
    st.job = Some(RunningJob {
        label: format!("{} {}", if redo { "Redo" } else { "Undo" }, action.describe()),
        workdir: Some(cwd.clone()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        keeps_undo: true,
    });
    fx.push(Effect::UndoRedo { cwd, action, redo });
}

/// Jobs that never move the local branch, HEAD, index or working tree, so the history stays valid.
pub(super) fn keeps_history(cmds: &[Vec<String>]) -> bool {
    cmds.iter().all(|c| matches!(c.first().map(String::as_str), Some("fetch" | "push" | "tag" | "clone" | "init")))
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
            // An undone commit's message comes back into an empty draft, ready to commit again.
            if let Action::Commit { message, amend: false, .. } = &before
                && draft.is_empty()
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
