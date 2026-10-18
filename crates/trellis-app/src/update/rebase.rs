//! Interactive rebase: opening the panel and loading its list, editing the list, starting the
//! rebase as the single active job, and showing where it ended (finished, stopped at an edit
//! or a conflict, or refused). Graph edits are in `rebase_quick`, the banner's Continue and
//! Skip in `rebase_resume`; their results end here too, in `finished`.

use super::ops::Started;
use super::rebase_resume::{STASH_KEPT, stopped_headline};
use crate::effect::{Effect, RebaseEffect};
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::state::rebase::{RebasePanel, needs_stash, open_blocker, start_question};
use crate::state::rebase_quick::QUICK_STALE;
use crate::state::{AppState, Selection};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::rebase::{self, Outcome, StartRequest};
use trellis_core::repo::Snapshot;

pub const JOB_LABEL: &str = "Interactive rebase";

pub(super) fn handle(st: &mut AppState, msg: RebaseMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        RebaseMsg::OpenRebase(base) => {
            let Some(snap) = &st.snap else { return };
            if let Some(why) = open_blocker(snap) {
                refuse(st, why, now);
                return;
            }
            super::repo::close_diff(st);
            super::conflict::close(st);
            super::history::close(st);
            super::blame::close(st);
            st.gitflow_panel = false;
            super::pull_request::close(st);
            super::issue::close(st);
            st.rebase = Some(RebasePanel::loading(base));
            request(st, fx);
        }
        RebaseMsg::RebasePlanLoaded { generation, result } => {
            if generation != st.rebase_gen || !st.rebase.as_ref().is_some_and(|p| p.loading) {
                return; // superseded: closed, reopened, or parked with its tab
            }
            match result {
                Ok(plan) => {
                    if let Some(p) = st.rebase.as_mut() {
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
            if !st.rebase.as_ref().is_some_and(|p| p.starting) {
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
        RebaseMsg::RebaseEditMessage(i) => {
            if let Some(p) = editable(st, i) {
                p.open_editor(i);
            }
        }
        RebaseMsg::RebaseSaveMessage => {
            if let Some(p) = st.rebase.as_mut().filter(|p| p.editable()) {
                p.save_message();
            }
        }
        RebaseMsg::RebaseCancelMessage => {
            if let Some(p) = st.rebase.as_mut() {
                p.close_editor();
            }
        }
        RebaseMsg::RebaseReset => {
            let Some(p) = st.rebase.as_mut().filter(|p| p.editable()) else { return };
            let stale = st.snap.as_ref().is_some_and(|s| p.is_stale(s));
            if stale {
                p.reload();
                request(st, fx);
            } else {
                p.reset();
            }
        }
        RebaseMsg::StartRebase { confirmed, autostash } => start(st, confirmed, autostash, now, fx),
        RebaseMsg::RebaseFinished { result } => finished(st, result, now, fx),
        RebaseMsg::Edit(e) => super::rebase_quick::edit(st, e, now, fx),
        RebaseMsg::EditPrepared { workdir, result } => super::rebase_quick::prepared(st, workdir, result, now, fx),
        RebaseMsg::SubmitReword => super::rebase_quick::submit_reword(st, now, fx),
        RebaseMsg::RunEdit { plan, autostash, confirmed } => {
            super::rebase_quick::run(st, *plan, autostash, confirmed, now, fx);
        }
        RebaseMsg::Resume(how) => super::rebase_resume::resume(st, how, now, fx),
    }
}

/// A refusal: logged, and shown as a red toast.
pub(super) fn refuse(st: &mut AppState, why: String, now: Instant) {
    st.log(false, why.clone());
    st.toast(false, why, now);
}

/// The open panel when it can be edited and `index` is one of its rows.
fn editable(st: &mut AppState, index: usize) -> Option<&mut RebasePanel> {
    st.rebase.as_mut().filter(|p| p.editable() && index < p.items.len())
}

/// Asks for the panel's list; results of older requests are ignored from now on.
fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.rebase_gen += 1;
    if let (Some(workdir), Some(p)) = (st.workdir(), &st.rebase) {
        fx.push(Effect::Rebase(RebaseEffect::LoadPlan { workdir, base: p.base.clone(), generation: st.rebase_gen }));
    }
}

/// Starts the panel's list. Drops and uncommitted changes ask first, in one question; a tree
/// that became dirty after a question that didn't mention stashing asks again.
fn start(st: &mut AppState, confirmed: bool, autostash: bool, now: Instant, fx: &mut Vec<Effect>) {
    let (Some(p), Some(snap)) = (&st.rebase, &st.snap) else { return };
    if let Some(why) = p.start_blocker(snap, st.job.is_some()) {
        st.log(false, why);
        return;
    }
    let drops = p.drops();
    let stash = needs_stash(snap);
    if (((drops > 0 || stash) && !confirmed) || (stash && !autostash))
        && let Some((title, body)) = start_question(drops, p.branch(), stash)
    {
        let then = Msg::Rebase(RebaseMsg::StartRebase { confirmed: true, autostash: stash });
        fx.extend(super::update(st, Msg::confirm(&title, body, "Start rebase", then), now));
        return;
    }
    let Some(plan) = p.plan.as_ref() else { return };
    let request = StartRequest { head: plan.head, upstream: plan.upstream, items: p.items.clone(), autostash: stash };
    // Also while it waits for the trust prompt: dropping it there resets `starting`.
    if launch(st, JOB_LABEL, request, fx) == Started::No {
        return;
    }
    if let Some(p) = st.rebase.as_mut() {
        p.starting = true;
        p.error = None;
    }
}

/// Runs `request` as the active job labelled `label`.
pub(super) fn launch(st: &mut AppState, label: &str, request: StartRequest, fx: &mut Vec<Effect>) -> Started {
    let started = super::ops::begin_job(st, label, false, move |cwd| Effect::Rebase(RebaseEffect::Start { cwd, request }), fx);
    if started != Started::No {
        super::submodule_batch::job_started(st, label, true);
    }
    started
}

/// A rebase job ended (the panel's, a graph edit's, or the banner's Continue / Skip). Its tab's
/// panel (active or parked) closes unless nothing was changed, in which case it stays open with
/// the reason. A stop selects the WIP row, where the banner is.
fn finished(st: &mut AppState, result: Result<Outcome, String>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = &st.job else { return };
    let (workdir, label) = (job.workdir.clone(), job.label.clone());
    let here = match (workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => trellis_core::paths::same_spelling(a, &b),
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
        st.rebase_gen += 1;
        // The banner with Continue / Skip / Abort is on the WIP row.
        if stopped && st.snap.is_some() {
            st.selection = Selection::Wip;
            st.detail = None;
        }
    }
    super::ops::finish_job(st, ok, output, headline, None, now, fx);
}

/// The panel of the tab at `workdir`: the open one when it is the active tab, otherwise the
/// one parked with it.
fn panel_slot(st: &mut AppState, workdir: Option<PathBuf>, here: bool) -> Option<&mut Option<RebasePanel>> {
    if here {
        return Some(&mut st.rebase);
    }
    let w = workdir?;
    st.tabs.iter_mut().find(|t| t.is(&w)).and_then(|t| t.parked.as_mut()).map(|p| &mut p.rebase)
}

/// After a refresh: the list is stale once HEAD or its branch moved.
pub(super) fn refreshed(st: &mut AppState, snap: &Snapshot) {
    if let Some(p) = st.rebase.as_mut()
        && p.plan.is_some()
        && p.is_stale(snap)
    {
        p.stale = true;
    }
}

/// Closes the panel; a list still loading must not reopen it.
pub(super) fn close(st: &mut AppState) {
    st.rebase = None;
    st.rebase_gen += 1;
}

/// After a tab switch: a list whose load was dropped while parked is requested again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.rebase.as_ref().is_some_and(|p| p.loading) {
        request(st, fx);
    }
}
