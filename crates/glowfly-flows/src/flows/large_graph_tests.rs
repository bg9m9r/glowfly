//! Headless tests for a large history's graph off the UI thread: the relayout when the work tree
//! turns dirty or clean, the hovered branch's rows, and freeing what a swap replaces, each giving
//! what the UI-thread path gives and ignoring answers for an older history.

use super::search_loaded_tests::{large_state, oid_of};
use super::tests::finish;
use super::update;
use crate::effect::{BackgroundEffect, Effect};
use crate::msg::{GraphMsg, Msg};
use crate::state::AppState;
use crate::state::refresh::LARGE_HISTORY;
use glowfly_core::repo::{FileEntry, Snapshot};
use std::sync::Arc;
use std::time::Instant;

/// What a layout shows, for comparing two of them.
fn shape(s: &Snapshot) -> impl PartialEq + std::fmt::Debug + use<> {
    let g = &s.graph;
    (g.wip.clone(), g.rows.clone(), g.worktree_wips.clone(), g.max_width, g.owners.commit.clone(), g.owners.wip, g.swatches.clone())
}

/// Whether the layout shown is the one the UI thread makes for the snapshot as it is now.
#[track_caller]
fn same_as_here(st: &mut AppState) {
    let snap = st.snap.as_mut().unwrap();
    let shown = shape(snap);
    snap.lay_out_graph();
    assert_eq!(shown, shape(snap));
}

fn relayouts(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Background(BackgroundEffect::Relayout { .. }))).count()
}

fn set_dirty(st: &mut AppState, dirty: bool) {
    st.snap.as_mut().unwrap().unstaged = if dirty { vec![FileEntry::new("a.txt", 'M')] } else { Vec::new() };
}

/// Any message, for the end-of-update check to run.
fn nudge(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::Graph(GraphMsg::HoverRef(None)), Instant::now())
}

#[test]
fn a_flip_is_laid_out_on_a_worker_like_on_the_ui_thread() {
    let (mut st, dir) = large_state("graph-flip", LARGE_HISTORY);
    assert!(nudge(&mut st).is_empty(), "nothing to do while the layout fits");
    set_dirty(&mut st, true);
    let fx = nudge(&mut st);
    assert_eq!(relayouts(&fx), 1, "{fx:?}");
    assert!(st.snap.as_ref().unwrap().graph.wip.is_none(), "not laid out on the UI thread");
    assert_eq!(relayouts(&nudge(&mut st)), 0, "one request at a time");
    finish(&mut st, fx);
    assert!(st.snap.as_ref().unwrap().graph.wip.is_some(), "the worker's layout is shown");
    same_as_here(&mut st);
    set_dirty(&mut st, false);
    let fx = nudge(&mut st);
    finish(&mut st, fx);
    assert!(st.snap.as_ref().unwrap().graph.wip.is_none());
    same_as_here(&mut st);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_layout_that_no_longer_fits_is_ignored() {
    let (mut st, dir) = large_state("graph-stale", LARGE_HISTORY);
    set_dirty(&mut st, true);
    let dirty = nudge(&mut st);
    // Clean again before the worker answers: its layout (with a WIP row) doesn't fit.
    set_dirty(&mut st, false);
    let before = shape(st.snap.as_ref().unwrap());
    finish(&mut st, dirty);
    assert_eq!(shape(st.snap.as_ref().unwrap()), before, "the dirty layout isn't shown");
    assert_eq!(st.refresh.relayout, None, "and nothing more is asked: the layout fits again");

    // A layout made for the history before a reload doesn't fit the new one.
    set_dirty(&mut st, true);
    let fx = nudge(&mut st);
    let snap = st.snap.as_mut().unwrap();
    snap.commits = snap.commits.to_vec().into();
    let fx2 = finish_one(&mut st, fx);
    assert!(st.snap.as_ref().unwrap().graph.wip.is_none(), "laid out for the old history");
    assert_eq!(relayouts(&fx2), 1, "asked again for the new one");
    finish(&mut st, fx2);
    same_as_here(&mut st);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs `fx` and delivers their results, returning what that update asked for.
fn finish_one(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let mut out = Vec::new();
    for e in fx {
        if let Some(m) = crate::runner::testing::execute(e) {
            out.extend(update(st, m, Instant::now()));
        }
    }
    out
}

#[test]
fn a_small_history_is_laid_out_at_once() {
    let (mut st, dir) = large_state("graph-small", 40);
    set_dirty(&mut st, true);
    super::refresh::status_set(st.snap.as_mut().unwrap(), false, None);
    assert!(st.snap.as_ref().unwrap().graph.wip.is_some());
    assert_eq!(relayouts(&nudge(&mut st)), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hovering_waits_for_the_ancestry_then_highlights_as_on_the_ui_thread() {
    let (mut st, dir) = large_state("graph-hover-large", LARGE_HISTORY);
    let tip = oid_of(12);
    let ancestry = |fx: &[Effect]| fx.iter().filter(|e| matches!(e, Effect::Background(BackgroundEffect::Ancestry { .. }))).count();
    let fx = update(&mut st, Msg::Graph(GraphMsg::HoverRef(Some(tip))), Instant::now());
    assert_eq!(ancestry(&fx), 1);
    assert!(st.view.highlight.is_none(), "nothing highlighted until the worker answers");
    assert_eq!(ancestry(&update(&mut st, Msg::Graph(GraphMsg::HoverRef(Some(tip))), Instant::now())), 0, "asked once");
    finish(&mut st, fx);
    update(&mut st, Msg::Graph(GraphMsg::HoverRef(Some(tip))), Instant::now());
    let (shown, rows) = st.view.highlight.clone().expect("highlighted once the ancestry is in");
    let snap = st.snap.as_ref().unwrap();
    let base = crate::state::ref_rows::default_ancestry(snap).1;
    assert_eq!(st.view.ref_rows.base.as_ref(), Some(&base), "the same ancestry as on the UI thread");
    assert_eq!((shown, &*rows), (tip, &crate::state::ref_rows::branch_rows(snap, tip, &base)));
    // Hovering it again reuses its rows.
    update(&mut st, Msg::Graph(GraphMsg::HoverRef(None)), Instant::now());
    update(&mut st, Msg::Graph(GraphMsg::HoverRef(Some(tip))), Instant::now());
    assert!(Arc::ptr_eq(&rows, &st.view.highlight.as_ref().unwrap().1), "worked out once per snapshot");

    // An answer for the snapshot before a reset is dropped.
    st.view.ref_rows.reset();
    let fx = update(&mut st, Msg::Graph(GraphMsg::HoverRef(Some(tip))), Instant::now());
    let request = st.view.ref_rows.asked.unwrap();
    update(&mut st, Msg::Graph(GraphMsg::AncestryFound { request: request - 1, base: Default::default() }), Instant::now());
    assert!(st.view.ref_rows.base.is_none(), "an older request's answer");
    finish(&mut st, fx);
    assert_eq!(st.view.ref_rows.base.as_ref(), Some(&base));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_replaced_large_snapshot_is_freed_on_a_worker() {
    let (mut st, dir) = large_state("graph-free", LARGE_HISTORY);
    let frees = |fx: &[Effect]| fx.iter().filter(|e| matches!(e, Effect::Background(BackgroundEffect::Free(_)))).count();
    let small = crate::state::op_fixture::snapshot("graph-free-small");
    let small_dir = small.workdir.clone();
    let mut fx = Vec::new();
    super::large_graph::show(&mut st, small, &mut fx);
    assert_eq!(frees(&fx), 1, "the large one goes to a worker");
    let mut fx = Vec::new();
    super::large_graph::show(&mut st, crate::state::op_fixture::snapshot("graph-free-small"), &mut fx);
    assert_eq!(frees(&fx), 0, "a small one is freed at once");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&small_dir);
}
