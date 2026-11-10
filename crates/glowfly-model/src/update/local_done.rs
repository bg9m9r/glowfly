//! A local op's result (`StageMsg::LocalOpDone`, `LocalOpFailed`): the failure worded in the
//! user's language for the log, the undo recorded, then the conflict view, the WIP list and the
//! status catch up.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::error_words::typed;
use glowfly_core::error::WriteFailed;
use glowfly_core::undo::Action;
use std::time::Instant;

/// A local op ended: a failure is logged, an undoable change recorded.
pub(super) fn done(st: &mut AppState, result: Result<Option<Action>, WriteFailed>, now: Instant, fx: &mut Vec<Effect>) {
    let failed = result.is_err();
    let recorded = matches!(result, Ok(Some(_)));
    match result {
        Err(e) => {
            let words = typed::write(st.settings.lang(), &e);
            st.log(false, words);
        }
        Ok(Some(action)) => st.session.undo.record(action),
        Ok(None) => {}
    }
    settle(st, failed, recorded, now, fx);
}

/// A conflict change that failed partway: its undo is recorded, and the log says Undo puts back
/// the files it changed.
pub(super) fn failed(st: &mut AppState, error: WriteFailed, undo: Action, now: Instant, fx: &mut Vec<Effect>) {
    st.session.undo.record(undo);
    let t = st.settings.lang();
    st.log(false, partial_text(t, &error));
    settle(st, true, false, now, fx);
}

/// The log line for a conflict change that failed partway with `error`.
pub(super) fn partial_text(t: glowfly_core::i18n::Lang, error: &WriteFailed) -> String {
    t.trf("undo.partial_conflict_change", &[("why", &typed::write(t, error))])
}

/// After a local op: a save that failed can be tried again; one that worked moves the conflict
/// view on once the status shows the file resolved (`recorded`: it changed the file, so its
/// toast offers Undo).
fn settle(st: &mut AppState, failed: bool, recorded: bool, now: Instant, fx: &mut Vec<Effect>) {
    let mut saved = None;
    // A conflict change that still waits for the trust prompt hasn't run: this is the result
    // of another operation, and the view stays marked for the change.
    let waits = super::trust_waiting::conflict_change_waits(st);
    if let Some(c) = st.session.conflict.view.as_mut().filter(|_| !waits) {
        c.saving = false;
        c.step_on_resolve &= !failed;
        // What Save with markers wrote is now what the file holds.
        if let Some((note, picks, cleans)) = c.pending_save.take().filter(|_| !failed) {
            c.baseline = (picks, cleans);
            saved = Some(note);
        }
    }
    // Save with markers leaves the view open: say it worked, with Undo when it changed the file.
    if let Some(note) = saved {
        let path = st.session.conflict.view.as_ref().map(|c| c.path.clone()).unwrap_or_default();
        let undo = super::conflict::undo_button(st, &path, false).filter(|_| recorded);
        super::conflict::toast_with(st, note, undo, now);
    }
    super::watch::settled(st, now);
    super::wip_list::op_done(st, !failed, fx);
    super::repo::refresh_status(st, fx);
}
