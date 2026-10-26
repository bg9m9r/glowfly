//! Interactive rebase: opening the panel and loading its list, editing the list, starting the
//! rebase as the single active job, and showing where it ended (finished, stopped at an edit
//! or a conflict, or refused). The panel on a running rebase's remaining steps is in
//! `rebase_remaining`, graph edits in `rebase_quick`, the banner's Continue and Skip in
//! `rebase_resume`, restacks in `stack`; rebase results end here, in `finished`.

use super::ops::Started;
use super::rebase_resume::{STASH_KEPT, stopped_headline};
use crate::effect::{Effect, RebaseEffect, UndoResult};
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::state::rebase::{PanelSource, RebasePanel, needs_stash, open_blocker, start_question};
use crate::state::rebase_quick::QUICK_STALE;
use crate::state::rebase_remaining::ENDED;
use crate::state::{AppState, Selection};
use git2::RepositoryState;
use glowfly_core::rebase::{self, Outcome, StartRequest};
use glowfly_core::repo::Snapshot;
use std::path::PathBuf;
use std::time::Instant;

pub const JOB_LABEL: &str = "Interactive rebase";

pub(super) fn handle(st: &mut AppState, msg: RebaseMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        RebaseMsg::OpenRebase(base) => {
            let Some(snap) = &st.snap else { return };
            if let Some(why) = open_blocker(snap) {
                refuse(st, why, now);
                return;
            }
            close_views(st);
            st.session.rebase.view = Some(RebasePanel::loading(base));
            request(st, fx);
        }
        RebaseMsg::RebasePlanLoaded { generation, result } => {
            if generation != st.session.rebase.generation || !st.session.rebase.view.as_ref().is_some_and(|p| p.loading) {
                return; // superseded: closed, reopened, or parked with its tab
            }
            match result {
                Ok(plan) => {
                    if let Some(p) = st.session.rebase.view.as_mut() {
                        p.fill(plan);
                    }
                }
                Err(e) => {
                    close(st);
                    refuse(st, e, now);
                }
            }
        }
        RebaseMsg::CloseRebase => {
            if !st.session.rebase.view.as_ref().is_some_and(|p| p.starting) {
                close(st);
            }
        }
        RebaseMsg::RebaseSetAction { index, action } => {
            if let Some(p) = editable(st, index) {
                p.set_action(index, action);
            }
        }
        RebaseMsg::RebaseMove { from, to } => {
            if let Some(p) = editable(st, from).filter(|p| to < p.items.len()) {
                p.move_item(from, to);
            }
        }
        RebaseMsg::RebaseSelect(i) => {
            let Some(p) = st.session.rebase.view.as_mut().filter(|p| i < p.items.len()) else { return };
            p.selected = Some(i);
            if let Some(oid) = p.items[i].oid() {
                super::repo::select_commit(st, oid, false, fx);
            }
        }
        RebaseMsg::RebaseAddStep { after, step } => {
            if let Some(p) = st.session.rebase.view.as_mut().filter(|p| p.editable()) {
                p.add_step(after, step);
            }
        }
        RebaseMsg::RebaseRemoveStep(i) => {
            if let Some(p) = editable(st, i) {
                p.remove_step(i);
            }
        }
        RebaseMsg::RebaseEditMessage(i) => {
            if let Some(p) = editable(st, i) {
                p.open_editor(i);
            }
        }
        RebaseMsg::RebaseSaveMessage => {
            if let Some(p) = st.session.rebase.view.as_mut().filter(|p| p.editable()) {
                p.save_message();
            }
        }
        RebaseMsg::RebaseCancelMessage => {
            if let Some(p) = st.session.rebase.view.as_mut() {
                p.close_editor();
            }
        }
        RebaseMsg::RebaseResetCombined => {
            let Some(p) = st.session.rebase.view.as_mut().filter(|p| p.editable()) else { return };
            if let Some(crate::state::rebase::Editing::Combined(h)) = p.editing {
                if let Some(item) = p.items.get_mut(h).and_then(|s| s.commit_mut()) {
                    item.combined_message = None;
                }
                p.close_editor();
            }
        }
        RebaseMsg::RebaseReset => {
            let Some(p) = st.session.rebase.view.as_mut().filter(|p| p.editable()) else { return };
            let stale = st.snap.as_ref().is_some_and(|s| p.is_stale(s));
            if stale {
                p.reload();
                request(st, fx);
            } else {
                p.reset();
            }
        }
        RebaseMsg::StartRebase { confirmed, autostash } => start(st, confirmed, autostash, now, fx),
        RebaseMsg::OpenRemaining => super::rebase_remaining::open(st, now, fx),
        RebaseMsg::RemainingLoaded { generation, result } => super::rebase_remaining::loaded(st, generation, result, now),
        RebaseMsg::RemainingEdited { result } => super::rebase_remaining::edited(st, result, now, fx),
        RebaseMsg::RebaseFinished { result, undo } => finished(st, result, undo, now, fx),
        RebaseMsg::Edit(e) => super::rebase_quick::edit(st, e, now, fx),
        RebaseMsg::EditPrepared { workdir, result } => super::rebase_quick::prepared(st, workdir, result, now, fx),
        RebaseMsg::SubmitReword => super::rebase_quick::submit_reword(st, now, fx),
        RebaseMsg::RunEdit { plan, autostash, confirmed } => {
            super::rebase_quick::run(st, *plan, autostash, confirmed, now, fx);
        }
        RebaseMsg::Resume(how) => super::rebase_resume::resume(st, how, now, fx),
        RebaseMsg::Restack { scope, autostash } => super::stack::restack(st, scope, autostash, now, fx),
        RebaseMsg::RestackRan { result, undo, job } => super::stack::ran(st, result, undo, *job, now, fx),
    }
}

/// A refusal: logged, and shown as a red toast.
pub(super) fn refuse(st: &mut AppState, why: String, now: Instant) {
    st.log(false, why.clone());
    st.toast(false, why, now);
}

/// Closes the views the panel replaces in the middle of the window.
pub(super) fn close_views(st: &mut AppState) {
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    st.session.gitflow_panel = false;
    super::journal::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
}

/// The open panel when it can be edited and `index` is one of its rows.
fn editable(st: &mut AppState, index: usize) -> Option<&mut RebasePanel> {
    st.session.rebase.view.as_mut().filter(|p| p.editable() && index < p.items.len())
}

/// Asks for the panel's list (a plan, or a running rebase's remaining steps); results of older
/// requests are ignored from now on.
pub(super) fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.session.rebase.generation += 1;
    let generation = st.session.rebase.generation;
    if let (Some(workdir), Some(p)) = (st.workdir(), &st.session.rebase.view) {
        fx.push(Effect::Rebase(match &p.source {
            PanelSource::Start(base) => RebaseEffect::LoadPlan { workdir, base: base.clone(), generation },
            PanelSource::Remaining => RebaseEffect::LoadRemaining { workdir, generation },
        }));
    }
}

/// Starts the panel's list. Drops and uncommitted changes ask first, in one question; a tree
/// that became dirty after a question that didn't mention stashing asks again. The remaining
/// steps of a running rebase are updated instead.
fn start(st: &mut AppState, confirmed: bool, autostash: bool, now: Instant, fx: &mut Vec<Effect>) {
    if st.session.rebase.view.as_ref().is_some_and(|p| p.source == PanelSource::Remaining) {
        return super::rebase_remaining::update_steps(st, confirmed, now, fx);
    }
    let (Some(p), Some(snap)) = (&st.session.rebase.view, &st.snap) else { return };
    if let Some(why) = p.start_blocker(snap, st.job.is_some()) {
        st.log(false, why);
        return;
    }
    let drops = p.drops();
    let stash = needs_stash(snap);
    // Always stash (Preferences > General) answers the stash-only question; drops still ask.
    let always = stash && st.settings.auto_stash;
    let autostash = autostash || always;
    if (((drops > 0 || (stash && !always)) && !confirmed) || (stash && !autostash))
        && let Some((title, body)) = start_question(drops, p.branch(), stash)
    {
        let then = Msg::Rebase(RebaseMsg::StartRebase { confirmed: true, autostash: stash });
        // Dropping commits loses them; only stashing first is an ordinary question.
        let ask = if drops > 0 {
            Msg::confirm_danger(&title, body, "Start rebase", then)
        } else {
            Msg::confirm(&title, body, "Start rebase", then)
        };
        fx.extend(super::update(st, ask, now));
        return;
    }
    let Some(plan) = p.plan.as_ref() else { return };
    let request = StartRequest { head: plan.head, upstream: plan.upstream, items: p.items.clone(), autostash: stash };
    let detail = format!("interactive rebase of {}", plan.branch);
    // Also while it waits for the trust prompt: dropping it there resets `starting`.
    if launch(st, JOB_LABEL, request, detail, fx) == Started::No {
        return;
    }
    if let Some(p) = st.session.rebase.view.as_mut() {
        p.starting = true;
        p.error = None;
    }
}

/// Runs `request` as the active job labelled `label`; `detail` names it in the undo history.
pub(super) fn launch(st: &mut AppState, label: &str, request: StartRequest, detail: String, fx: &mut Vec<Effect>) -> Started {
    let started = super::ops::begin_job(st, label, false, move |cwd| Effect::Rebase(RebaseEffect::Start { cwd, request, detail }), fx);
    if started != Started::No {
        super::submodule_batch::job_started(st, label, true);
    }
    started
}

/// A rebase job ended (the panel's, a graph edit's, or the banner's Continue / Skip). Its tab's
/// panel (active or parked) closes unless nothing was changed, in which case it stays open with
/// the reason. A stop selects the WIP row, where the banner is. `undo` is what the run means
/// for the undo history.
pub(super) fn finished(st: &mut AppState, result: Result<Outcome, String>, undo: Option<UndoResult>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = &st.job else { return };
    let (workdir, label) = (job.workdir.clone(), job.label.clone());
    let here = match (workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => glowfly_core::paths::same_spelling(a, &b),
        _ => false,
    };
    let panel_open = panel_slot(st, workdir.clone(), here).is_some_and(|s| s.is_some());
    // `keep`: the panel stays open with this error (and is marked stale when nothing ran).
    let (ok, output, headline, keep, stopped) = match result {
        Ok(Outcome::Finished { output, stash_kept }) => {
            let headline = stash_kept.then(|| format!("{label} {STASH_KEPT}"));
            (!stash_kept, output, headline, None, false)
        }
        Ok(Outcome::Stopped { ok, output, progress }) => {
            let headline = stopped_headline(&label, &progress, ok, &output);
            (ok, output, Some(headline), None, true)
        }
        Ok(Outcome::Failed { output }) => {
            let error = super::ops::last_line(&output).to_owned();
            (false, output, None, Some((error, false)), false)
        }
        Err(e) => {
            // Only the panel can be reset to reload its list.
            let shown = if e == rebase::STALE && !panel_open { QUICK_STALE } else { e.as_str() };
            let headline = format!("{label} failed: {shown}");
            (false, e.clone(), Some(headline), Some((e, true)), false)
        }
    };
    let closes = keep.is_none();
    if let Some(slot) = panel_slot(st, workdir, here) {
        match keep {
            Some((error, stale)) => {
                if let Some(p) = slot.as_mut() {
                    p.starting = false;
                    p.error = Some(error);
                    p.stale |= stale;
                }
            }
            None => *slot = None,
        }
    }
    if here && closes {
        st.session.rebase.generation += 1;
        // The banner with Continue / Skip / Abort is on the WIP row.
        if stopped && st.snap.is_some() {
            st.selection = Selection::Wip;
            st.detail = None;
        }
    }
    super::ops::finish_job(st, ok, output, headline, undo, now, fx);
}

/// The panel of the tab at `workdir`: the open one when it is the active tab, otherwise the
/// one parked with it.
pub(super) fn panel_slot(st: &mut AppState, workdir: Option<PathBuf>, here: bool) -> Option<&mut Option<RebasePanel>> {
    if here {
        return Some(&mut st.session.rebase.view);
    }
    let w = workdir?;
    st.tabs.iter_mut().find(|t| t.is(&w)).and_then(|t| t.parked.as_mut()).map(|p| &mut p.session.rebase.view)
}

/// After a refresh: the list is stale once HEAD or its branch moved, or the rebase whose steps
/// it shows moved on; that rebase ending closes the panel.
pub(super) fn refreshed(st: &mut AppState, snap: &Snapshot) {
    let Some(p) = st.session.rebase.view.as_mut() else { return };
    if p.source == PanelSource::Remaining && !p.starting && snap.state != RepositoryState::RebaseInteractive {
        close(st);
        st.log(false, ENDED);
        return;
    }
    if p.loaded() && p.is_stale(snap) {
        p.stale = true;
    }
}

/// Closes the panel; a list still loading must not reopen it.
pub(super) fn close(st: &mut AppState) {
    st.session.rebase.view = None;
    st.session.rebase.generation += 1;
}

/// After a tab switch: a list whose load was dropped while parked is requested again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.session.rebase.view.as_ref().is_some_and(|p| p.loading) {
        request(st, fx);
    }
}
