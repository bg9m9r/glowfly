//! History edits from the graph's menus: preparing the rebase (`Effect::Rebase(PrepareEdit)`),
//! asking first (the Reword dialog, the drop confirmation, stashing uncommitted changes), and
//! running it as a rebase job whose result ends in `rebase::finished`.

use super::ops::Started;
use super::rebase::{launch, refuse};
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::{JOB_RUNNING, STASH_LINE, needs_stash, open_blocker};
use crate::state::rebase_quick::{RewordForm, drop_question, label, squash_question};
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::rebase::{QuickEdit, QuickPlan};

/// A menu entry was chosen: prepare its rebase, unless nothing can be edited now.
pub(super) fn edit(st: &mut AppState, edit: QuickEdit, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    if let Some(why) = open_blocker(snap) {
        refuse(st, why, now);
        return;
    }
    if st.job.is_some() {
        refuse(st, JOB_RUNNING.into(), now);
        return;
    }
    if let Some(workdir) = st.workdir() {
        fx.push(Effect::Rebase(RebaseEffect::PrepareEdit { workdir, edit }));
    }
}

/// The prepared edit: a reword opens its dialog, a drop asks, a squash runs (asking first on a
/// dirty tree). Ignored for another tab, or when a dialog opened meanwhile.
pub(super) fn prepared(st: &mut AppState, workdir: PathBuf, result: Result<QuickPlan, String>, now: Instant, fx: &mut Vec<Effect>) {
    if !st.workdir().is_some_and(|w| same_spelling(&w, &workdir)) || st.modal.is_some() {
        return;
    }
    let plan = match result {
        Ok(p) => p,
        Err(e) => return refuse(st, e, now),
    };
    let stash = st.snap.as_ref().is_some_and(needs_stash);
    match plan.edit {
        QuickEdit::Reword(_) => st.modal = Some(Modal::RewordCommit(Box::new(RewordForm::new(workdir, plan)))),
        QuickEdit::Drop(_) => ask_drop(st, plan, stash, now, fx),
        QuickEdit::SquashIntoParent(_) | QuickEdit::Squash(_) => {
            run(st, plan, false, false, now, fx);
        }
    }
}

/// The Reword dialog's primary button: runs the typed message. The dialog was the question
/// (it says when the changes are stashed), and it closes once the job started.
pub(super) fn submit_reword(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(Modal::RewordCommit(form)) = &st.modal else { return };
    if form.blocker(true).is_some() || !st.workdir().is_some_and(|w| same_spelling(&w, &form.workdir)) {
        return;
    }
    let mut plan = form.plan.clone();
    let i = form.index();
    plan.request.items[i].new_message = Some(form.draft.trim_end().to_owned());
    let stash = st.snap.as_ref().is_some_and(needs_stash);
    if run(st, plan, stash, true, now, fx) != Started::No && matches!(st.modal, Some(Modal::RewordCommit(_))) {
        st.modal = None;
    }
}

/// Runs `plan`, unless it must ask first: a drop that wasn't confirmed, or uncommitted changes
/// the last question didn't mention (`autostash` false).
pub(super) fn run(st: &mut AppState, mut plan: QuickPlan, autostash: bool, confirmed: bool, now: Instant, fx: &mut Vec<Effect>) -> Started {
    let stash = st.snap.as_ref().is_some_and(needs_stash);
    let is_drop = matches!(plan.edit, QuickEdit::Drop(_));
    if (is_drop && !confirmed) || (stash && !autostash) {
        if is_drop {
            ask_drop(st, plan, stash, now, fx);
        } else {
            let (title, body, button) = match plan.edit {
                QuickEdit::Reword(_) => ("Stash changes and reword".to_owned(), STASH_LINE.to_owned(), "Reword".to_owned()),
                _ => squash_question(&plan),
            };
            let then = Msg::Rebase(RebaseMsg::RunEdit { plan: Box::new(plan), autostash: true, confirmed: true });
            fx.extend(super::update(st, Msg::confirm(&title, body, &button, then), now));
        }
        return Started::No;
    }
    plan.request.autostash = stash;
    launch(st, label(&plan.edit), plan.request, fx)
}

fn ask_drop(st: &mut AppState, plan: QuickPlan, stash: bool, now: Instant, fx: &mut Vec<Effect>) {
    let (title, body, button) = drop_question(&plan, stash);
    let then = Msg::Rebase(RebaseMsg::RunEdit { plan: Box::new(plan), autostash: stash, confirmed: true });
    fx.extend(super::update(st, Msg::confirm(&title, body, &button, then), now));
}
