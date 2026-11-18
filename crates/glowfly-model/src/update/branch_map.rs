//! The Branch map (`BranchMapMsg`): opening it in place of the graph, asking for the read, its
//! result, the selection and closing it. Every read carries a generation from
//! `RepoSession::branch_map`, so a result for a view that was closed, replaced or parked is
//! dropped, and the running read is asked to stop whenever one of those happens. A refresh reads
//! again only when the plan made from the new snapshot differs from the one on screen; the old
//! map stays up meanwhile. Reading needs no trust: libgit2 only, nothing is written.

use crate::branch_map::{BranchMapJob, BranchMapMsg};
use crate::effect::partial::StopKey;
use crate::effect::{ControlEffect, Effect};
use crate::state::AppState;
use crate::state::branch_map::BranchMapView;
use glowfly_core::branch_map::{self, BranchMap, Empty};
use glowfly_core::error::ReadFailed;
use std::sync::Arc;

pub(super) fn handle(st: &mut AppState, msg: BranchMapMsg, fx: &mut Vec<Effect>) {
    match msg {
        BranchMapMsg::Open => open(st, fx),
        BranchMapMsg::Close => close(st, fx),
        BranchMapMsg::Retry => load(st, fx),
        BranchMapMsg::Select(branch) => {
            if let Some(view) = st.session.branch_map.view.as_mut()
                && view.map().is_some_and(|m| m.row_of(&branch).is_some())
            {
                view.selected = Some(branch);
            }
        }
        BranchMapMsg::Loaded { generation, result } => loaded(st, generation, result),
    }
}

/// Opens the map in place of the graph, closing the views it covers, and reads it.
fn open(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.snap.is_none() || st.session.rebase.view.as_ref().is_some_and(|p| p.starting) {
        return;
    }
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    super::rebase::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    super::range_diff::close(st);
    super::journal::close(st);
    st.session.gitflow_panel = false;
    if st.session.branch_map.view.is_none() {
        let view = BranchMapView { generation: 0, loading: false, plan: Err(Empty::NoCommits), result: None, selected: None };
        st.session.branch_map.view = Some(view);
    }
    load(st, fx);
}

/// Stops the running read, plans from the snapshot and asks for a new one.
fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    stop_running(st, fx);
    ask(st, fx);
}

/// Asks for a read of what the snapshot says to measure, under a new generation. The result on
/// screen stays until the new one arrives.
fn ask(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(workdir), Some(snap)) = (st.workdir(), st.snap.as_ref()) else { return };
    let plan = branch_map::request(snap);
    let slot = &mut st.session.branch_map;
    slot.generation += 1;
    let generation = slot.generation;
    let Some(view) = slot.view.as_mut() else { return };
    view.generation = generation;
    view.loading = plan.is_ok();
    if plan.is_err() {
        view.result = None;
    }
    view.plan = plan.clone();
    if let Ok(request) = plan {
        fx.push(Effect::BranchMap(BranchMapJob { workdir, request, known: Arc::clone(&slot.known), generation }));
    }
}

/// Asks the running read to stop.
fn stop_running(st: &AppState, fx: &mut Vec<Effect>) {
    if let Some(view) = st.session.branch_map.view.as_ref().filter(|v| v.loading) {
        fx.push(Effect::Control(ControlEffect::StopGit(StopKey::BranchMap(view.generation))));
    }
}

fn loaded(st: &mut AppState, generation: u64, result: Result<Box<BranchMap>, ReadFailed>) {
    if let Ok(map) = &result
        && st.session.branch_map.view.as_ref().is_some_and(|v| v.generation == generation)
    {
        st.session.branch_map.remember(map);
    }
    let Some(view) = st.session.branch_map.view.as_mut().filter(|v| v.generation == generation) else { return };
    view.loading = false;
    if let Ok(map) = &result {
        let kept = view.selected.take().filter(|b| map.row_of(b).is_some());
        view.selected = kept.or_else(|| map.current.map(|i| map.rows[i].branch.clone()));
    }
    view.result = Some(result);
}

/// Back to what was underneath: stops the read, drops the view and what is in flight.
pub(super) fn close(st: &mut AppState, fx: &mut Vec<Effect>) {
    stop_running(st, fx);
    st.session.branch_map.view = None;
    st.session.branch_map.generation += 1;
}

/// After a refresh: reads again when the branches the map would measure changed (a tip, the
/// set of branches, the checked-out branch or the main branch).
pub(super) fn refreshed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(view)) = (st.snap.as_ref(), st.session.branch_map.view.as_ref()) else { return };
    if branch_map::request(snap) != view.plan {
        load(st, fx);
    }
}

/// On a tab's return: asks again for a read that was in flight when it was parked.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.session.branch_map.view.as_ref().is_some_and(|v| v.loading) {
        ask(st, fx);
    }
}

/// Leaving the tab (a switch or a close): the running read is asked to stop. The view stays
/// loading, so `reload_pending` asks again on return.
pub(super) fn leave(st: &mut AppState, fx: &mut Vec<Effect>) {
    stop_running(st, fx);
}

/// The tab opened another repository: the map and what was learned about the old one go.
pub(super) fn repo_opened(st: &mut AppState, fx: &mut Vec<Effect>) {
    close(st, fx);
    st.session.branch_map.forget();
}
