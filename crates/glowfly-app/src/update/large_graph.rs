//! The graph of a large history (`refresh::LARGE_HISTORY`) without whole-history work on the UI
//! thread: when its WIP row comes or goes, or another worktree's, it is laid out on a worker
//! (`BackgroundEffect::Relayout`) and the layout swapped in when it still fits; the hovered
//! branch's rows wait for the default branch's ancestry from a worker; and what a swap replaces
//! is freed on a worker (`BackgroundEffect::Free`). A smaller history does all of it at once.

use crate::effect::{BackgroundEffect, Discard, Effect, Shared};
use crate::state::AppState;
use crate::state::refresh::large;
use git2::Oid;
use glowfly_core::graph::Layout;
use glowfly_core::repo::branch_colours::LayoutFit;
use std::collections::HashSet;
use std::sync::Arc;

/// After every update: a large history whose WIP row no longer matches the work tree (status
/// reads skip the relayout for it) is laid out again on a worker, one request at a time.
pub(super) fn settle(st: &mut AppState, fx: &mut Vec<Effect>) {
    let stale = st.snap.as_ref().is_some_and(|s| large(s) && s.graph.wip.is_some() != s.is_dirty());
    if stale && st.refresh.relayout.is_none() {
        relayout(st, fx);
    }
}

/// Lays the graph out again: at once for a small history, else on a worker.
pub(super) fn relayout(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_mut() else { return };
    if !large(snap) {
        snap.lay_out_graph();
        return;
    }
    st.refresh.relayouts += 1;
    st.refresh.relayout = Some(st.refresh.relayouts);
    let job = snap.layout_job();
    fx.push(Effect::Background(BackgroundEffect::Relayout { workdir: snap.workdir.clone(), request: st.refresh.relayouts, job }));
}

/// A worker's layout for request `request`: shown when it still fits the snapshot (an older one
/// that fits is as good), the replaced one freed on a worker.
pub(crate) fn laid_out(st: &mut AppState, request: u64, fit: &LayoutFit, layout: Layout, fx: &mut Vec<Effect>) {
    if st.refresh.relayout == Some(request) {
        st.refresh.relayout = None;
    }
    let Some(snap) = st.snap.as_mut().filter(|s| fit.fits(s)) else { return };
    let old = snap.set_layout(layout);
    free(fx, "layout", Box::new(old));
}

/// Shows the snapshot a load made; the one it replaces is freed on a worker when large.
pub(crate) fn show(st: &mut AppState, snap: glowfly_core::repo::Snapshot, fx: &mut Vec<Effect>) {
    match st.snap.replace(snap) {
        Some(old) if large(&old) => free(fx, "snapshot", Box::new(old)),
        _ => {}
    }
}

/// Hands `what` to a worker to free.
pub(super) fn free(fx: &mut Vec<Effect>, name: &'static str, what: Box<dyn Send>) {
    fx.push(Effect::Background(BackgroundEffect::Free(Discard(name, what))));
}

/// The default branch's ancestry, needed for highlighting: walked at once for a small history;
/// for a large one asked of a worker (once) and false until it answers.
pub(super) fn ancestry_ready(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let (Some(snap), rows) = (st.snap.as_ref(), &mut st.view.ref_rows) else { return false };
    if rows.base.is_some() {
        return true;
    }
    if !large(snap) {
        rows.base = Some(crate::state::ref_rows::default_ancestry(snap).1);
        return true;
    }
    if rows.asked.is_none() {
        rows.requests += 1;
        rows.asked = Some(rows.requests);
        let tip = crate::state::ref_rows::default_tip(snap);
        let (workdir, commits) = (snap.workdir.clone(), Shared(Arc::clone(&snap.commits)));
        fx.push(Effect::Background(BackgroundEffect::Ancestry { workdir, request: rows.requests, commits, tip }));
    }
    false
}

/// A worker's ancestry for request `request`: kept when it is the one asked for (the snapshot is
/// the same); the graph asks for the hovered branch's rows again on its next frame.
pub(super) fn ancestry_found(st: &mut AppState, request: u64, base: HashSet<Oid>) {
    let rows = &mut st.view.ref_rows;
    if rows.asked == Some(request) {
        rows.asked = None;
        rows.base = Some(base);
    }
}
