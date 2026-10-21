//! The rebase panel on the remaining steps of a running interactive rebase: opening it, its
//! list arriving, and Update steps running `git rebase --edit-todo` as the active job. The
//! rebase stays stopped where it was, so the undo history is kept.

use super::ops::Started;
use super::rebase::{close, close_views, panel_slot, refuse, request};
use crate::effect::{Effect, RebaseEffect, UndoResult};
use crate::msg::{Msg, RebaseMsg};
use crate::state::AppState;
use crate::state::rebase::{RebasePanel, start_question};
use crate::state::rebase_remaining::{EDIT_JOB, UPDATE, UPDATED, edit_request, open_remaining_blocker};
use glowfly_core::rebase::{self, Remaining};
use std::time::Instant;

/// "Edit remaining steps…": opens the panel in place of the graph and asks for the steps.
pub(super) fn open(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    if let Some(why) = open_remaining_blocker(snap) {
        refuse(st, why, now);
        return;
    }
    if st.rebase.as_ref().is_some_and(|p| p.starting) {
        return;
    }
    close_views(st);
    st.rebase = Some(RebasePanel::loading_remaining());
    request(st, fx);
}

/// The steps arrived; a refusal closes the panel and says why.
pub(super) fn loaded(st: &mut AppState, generation: u64, result: Result<Remaining, String>, now: Instant) {
    if generation != st.rebase_gen || !st.rebase.as_ref().is_some_and(|p| p.loading) {
        return; // superseded: closed, reopened, or parked with its tab
    }
    match result {
        Ok(r) => {
            if let Some(p) = st.rebase.as_mut() {
                p.fill_remaining(r);
            }
        }
        Err(e) => {
            close(st);
            refuse(st, e, now);
        }
    }
}

/// Update steps: newly dropped commits ask first; then the edit runs as the active job.
pub(super) fn update_steps(st: &mut AppState, confirmed: bool, now: Instant, fx: &mut Vec<Effect>) {
    let (Some(p), Some(snap)) = (&st.rebase, &st.snap) else { return };
    if let Some(why) = p.start_blocker(snap, st.job.is_some()) {
        st.log(false, why);
        return;
    }
    let drops = p.drops();
    if drops > 0
        && !confirmed
        && let Some((title, body)) = start_question(drops, p.branch(), false)
    {
        let then = Msg::Rebase(RebaseMsg::StartRebase { confirmed: true, autostash: false });
        fx.extend(super::update(st, Msg::confirm_danger(&title, body, UPDATE, then), now));
        return;
    }
    let Some(request) = edit_request(p) else { return };
    let started = super::ops::begin_job(st, EDIT_JOB, false, move |cwd| Effect::Rebase(RebaseEffect::EditTodo { cwd, request }), fx);
    if started == Started::No {
        return;
    }
    if let Some(p) = st.rebase.as_mut() {
        p.starting = true;
        p.error = None;
    }
}

/// The edit-todo job ended: its tab's panel closes, or stays open with the reason (marked
/// stale when the rebase had moved on).
pub(super) fn edited(st: &mut AppState, result: Result<(), String>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = &st.job else { return };
    let workdir = job.workdir.clone();
    let here = match (workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => glowfly_core::paths::same_spelling(a, &b),
        _ => false,
    };
    let ok = result.is_ok();
    if let Some(slot) = panel_slot(st, workdir, here) {
        match &result {
            Ok(()) => *slot = None,
            Err(e) => {
                if let Some(p) = slot.as_mut() {
                    p.starting = false;
                    p.error = Some(e.clone());
                    p.stale |= e == rebase::TODO_MOVED;
                }
            }
        }
    }
    if here && ok {
        st.rebase_gen += 1;
    }
    let (headline, output) = match result {
        Ok(()) => (UPDATED.to_owned(), String::new()),
        Err(e) => (format!("Couldn't update the rebase steps: {e}"), e),
    };
    super::ops::finish_job(st, ok, output, Some(headline), Some(UndoResult::Recorded(None)), now, fx);
}
