//! The Range-diff view (`RangeDiffMsg`): opening it in the central area from the branch
//! menus, the journal or the multi-commit menu, asking for the comparison, its result, the
//! selection and closing it. Every request carries a generation from
//! `RepoSession::range_diff`, so a result for a view that was closed, reopened or parked
//! meanwhile is dropped. In a repository held by trust nothing runs: the view says why and the
//! trust prompt is asked, and `trust_released` runs the comparison once it is trusted. The two
//! ends are fixed commits, so a refresh never reloads it.

use crate::effect::Effect;
use crate::msg::{Msg, TrustMsg};
use crate::range_diff::{RangeDiffJob, RangeDiffMsg};
use crate::state::AppState;
use crate::state::range_diff::{self as rd, Origin, Phase, RangeDiffView};
use glowfly_core::git_version::Feature;
use glowfly_core::range_diff::{Ends, Outcome};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: RangeDiffMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        RangeDiffMsg::Open { old, new, label, at } => {
            if old == new {
                let text = st.settings.lang().tr("range_diff.blocked.same_commit");
                st.toast(false, text, now);
                st.log(false, text);
                return;
            }
            open(st, Origin { ends: Ends::Tips { old, new }, label, at }, now, fx);
        }
        RangeDiffMsg::OpenRewrite(refname) => {
            let label = Some(rd::short_ref(&refname).to_owned());
            open(st, Origin { ends: Ends::LastRewrite { refname }, label, at: None }, now, fx);
        }
        RangeDiffMsg::Close => close(st),
        RangeDiffMsg::Retry => retry(st, now, fx),
        RangeDiffMsg::Select(i) => {
            if let Some(view) = st.session.range_diff.view.as_mut()
                && view.compared().is_some_and(|c| i < c.pairs.len())
            {
                view.selected = Some(i);
            }
        }
        RangeDiffMsg::Loaded { generation, result } => loaded(st, generation, result),
    }
}

/// Opens the view in place of the graph, closing the views it covers (the journal and the Git
/// Flow panel stay underneath), and asks for the comparison.
fn open(st: &mut AppState, origin: Origin, now: Instant, fx: &mut Vec<Effect>) {
    if st.snap.is_none() || st.session.rebase.view.as_ref().is_some_and(|p| p.starting) {
        return;
    }
    if !super::git_env::allows(st, Feature::RangeDiff, now) {
        return;
    }
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    super::rebase::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    st.session.range_diff.generation += 1;
    st.session.range_diff.view = Some(RangeDiffView::loading(origin, st.session.range_diff.generation));
    start(st, now, fx);
}

/// Runs the comparison, or holds it until the repository is trusted.
fn start(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if st.runs_git_unasked() {
        return request(st, fx);
    }
    if let Some(view) = st.session.range_diff.view.as_mut() {
        view.phase = Phase::Held;
    }
    if let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) {
        fx.extend(super::update(st, Msg::Trust(TrustMsg::Ask { real }), now));
    }
}

/// Asks for the open view's comparison under a new generation.
fn request(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(workdir), Some(snap)) = (st.workdir(), st.snap.as_ref()) else { return };
    let Some(origin) = st.session.range_diff.view.as_ref().map(|v| v.origin.clone()) else { return };
    let mainline = rd::mainline_for(snap, &origin);
    st.session.range_diff.generation += 1;
    let generation = st.session.range_diff.generation;
    let Some(view) = st.session.range_diff.view.as_mut() else { return };
    view.generation = generation;
    view.phase = Phase::Loading;
    view.selected = None;
    fx.push(Effect::RangeDiff(RangeDiffJob { workdir, ends: origin.ends, mainline, generation }));
}

fn retry(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if st.session.range_diff.view.is_none() {
        return;
    }
    st.session.range_diff.generation += 1;
    if let Some(view) = st.session.range_diff.view.as_mut() {
        view.generation = st.session.range_diff.generation;
        view.phase = Phase::Loading;
    }
    start(st, now, fx);
}

fn loaded(st: &mut AppState, generation: u64, result: Result<Box<Outcome>, String>) {
    let tab = st.settings.tab_columns();
    let Some(view) = st.session.range_diff.view.as_mut().filter(|v| v.generation == generation) else { return };
    match result {
        Ok(outcome) => {
            view.phase = Phase::Ready(outcome);
            view.selected = view.compared().and_then(rd::first_changed);
            view.widest = view.compared().map(|c| rd::widest(c, tab)).unwrap_or_default();
        }
        Err(e) => view.phase = Phase::Failed(e),
    }
}

/// Back to what was underneath.
pub(super) fn close(st: &mut AppState) {
    st.session.range_diff.view = None;
}

/// On a tab's return: asks again for a comparison that was in flight when it was parked.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.session.range_diff.view.as_ref().is_some_and(|v| v.phase == Phase::Loading) && st.runs_git_unasked() {
        request(st, fx);
    }
}

/// Once the repository is trusted: runs the comparison that was held.
pub(super) fn trust_released(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.session.range_diff.view.as_ref().is_some_and(|v| v.phase == Phase::Held) {
        request(st, fx);
    }
}
