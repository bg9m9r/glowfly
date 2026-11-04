//! Headless tests for freeing a long-parked tab's history (`update/parked_free.rs`): only a
//! large history parked `HISTORY_FREE_AFTER` is freed, on a worker; shown again, the tab reloads
//! it in full and as deep, keeping the selection and the scroll, or shows why it couldn't with
//! Try again; and a small history stays.

use super::tests::{long_repo, scratch_repo, state};
use super::update;
use crate::effect::{BackgroundEffect, Effect, LoadKind};
use crate::msg::{Msg, RepoMsg, TabsMsg};
use crate::state::tabs::{HISTORY_FREE_AFTER, HISTORY_FREE_MIN_COMMITS, Parked, history_free_wake};
use crate::state::{AppState, Selection, Tab};
use glowfly_core::repo::{self, LoadOptions, Snapshot};
use std::path::Path;
use std::time::{Duration, Instant};

fn tick(st: &mut AppState, t: Instant) -> Vec<Effect> {
    update(st, Msg::Tick(t), Instant::now())
}

fn load(dir: &Path, n: usize) -> Snapshot {
    repo::load(dir, &LoadOptions::all(n)).unwrap()
}

/// A state whose only tab shows `dir` loaded `n` deep, with commit `row` selected and the graph
/// scrolled to `scroll`, then parked behind a new empty tab.
fn parked_behind_new_tab(dir: &Path, n: usize, row: usize, scroll: f32) -> AppState {
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.to_path_buf());
    let snap = load(dir, n);
    st.session.selection = Selection::Commit(snap.commits[row].oid);
    st.snap = Some(snap);
    st.view.tab.graph_scroll = (scroll, 400.0);
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert_eq!(st.active_tab, 1);
    st
}

fn parked(st: &AppState, i: usize) -> &Parked {
    st.tabs[i].parked.as_deref().expect("a parked tab")
}

fn frees_on_a_worker(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Background(BackgroundEffect::Free(_))))
}

#[test]
fn a_long_parked_large_history_is_freed_and_reloaded_as_it_was() {
    let n = HISTORY_FREE_MIN_COMMITS;
    let dir = long_repo("parked-free-large", n + 20);
    let mut st = parked_behind_new_tab(&dir, n, 3_000, 75_000.0);
    let oid = st.tabs[0].parked.as_ref().and_then(|p| p.snap.as_ref()).map(|s| s.commits[3_000].oid).unwrap();

    let t0 = Instant::now();
    assert_eq!(history_free_wake(&st.tabs, t0), Some(t0), "a tick dates the park");
    tick(&mut st, t0);
    assert_eq!(history_free_wake(&st.tabs, t0), Some(t0 + HISTORY_FREE_AFTER));
    let fx = tick(&mut st, t0 + HISTORY_FREE_AFTER - Duration::from_secs(1));
    assert!(!frees_on_a_worker(&fx));
    assert_eq!(parked(&st, 0).snap.as_ref().unwrap().commits.len(), n, "not parked long enough yet");

    let fx = tick(&mut st, t0 + HISTORY_FREE_AFTER);
    assert!(frees_on_a_worker(&fx), "the history is freed on a worker");
    let p = parked(&st, 0);
    let stub = p.snap.as_ref().unwrap();
    assert!(stub.commits.is_empty() && stub.row_of.is_empty() && stub.graph.rows.is_empty());
    assert!(stub.head_oid.is_some() && !stub.locals.is_empty(), "the refs stay");
    assert_eq!(p.session.tab_view.history_freed, Some(n));
    assert_eq!(history_free_wake(&st.tabs, t0 + HISTORY_FREE_AFTER), None, "nothing more to free");
    assert!(!frees_on_a_worker(&tick(&mut st, t0 + 2 * HISTORY_FREE_AFTER)), "freed once");

    // Shown again: a full reload as deep as before, with the selection and the scroll kept.
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let reload = fx.iter().find_map(|e| match e {
        Effect::Load(crate::effect::LoadEffect::Repo { kind, known, prior, opts, .. }) => {
            Some((*kind, known.is_none(), prior.is_none(), opts.max_commits))
        }
        _ => None,
    });
    assert_eq!(reload, Some((LoadKind::Refresh, true, true, n)), "{fx:?}");
    assert_eq!(st.session.selection, Selection::Commit(oid));
    assert_eq!(st.view.scroll_offset, Some(75_000.0));
    assert!(st.loading);

    // A refresh asked for meanwhile stays a full one.
    update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    let (result, generation) = (Ok(Box::new(load(&dir, n))), st.load_gen);
    let fx = update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Refresh, result }), Instant::now());
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), n, "the history is back");
    assert_eq!(st.session.tab_view.history_freed, None);
    assert_eq!(st.session.selection, Selection::Commit(oid), "the selected commit is still loaded");
    assert_eq!(st.view.scroll_offset, Some(75_000.0), "the graph scrolls back once drawn");
    assert!(
        fx.iter().all(|e| !matches!(e, Effect::Load(crate::effect::LoadEffect::Repo { known: Some(_), .. }))),
        "no check of the walk: {fx:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_small_history_or_a_short_park_keeps_its_commits() {
    let dir = scratch_repo("parked-free-small");
    let mut st = parked_behind_new_tab(&dir, 100, 0, 0.0);
    let t0 = Instant::now();
    tick(&mut st, t0);
    assert_eq!(history_free_wake(&st.tabs, t0), None, "a small history is never freed");
    let fx = tick(&mut st, t0 + 3 * HISTORY_FREE_AFTER);
    assert!(!frees_on_a_worker(&fx));
    assert_eq!(parked(&st, 0).snap.as_ref().unwrap().commits.len(), 1);
    assert_eq!(parked(&st, 0).session.tab_view.history_freed, None);

    // Shown again, it refreshes as usual, taking the commits it holds.
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(crate::effect::LoadEffect::Repo { prior: Some(_), .. }))), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_park_cut_short_starts_counting_again() {
    let n = HISTORY_FREE_MIN_COMMITS;
    let dir = long_repo("parked-free-again", n);
    let mut st = parked_behind_new_tab(&dir, n, 0, 0.0);
    let t0 = Instant::now();
    tick(&mut st, t0);
    // Back for a moment, then parked again: the time counts from the new park.
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    let t1 = t0 + HISTORY_FREE_AFTER - Duration::from_secs(60);
    tick(&mut st, t1);
    assert!(!frees_on_a_worker(&tick(&mut st, t0 + HISTORY_FREE_AFTER)));
    assert!(frees_on_a_worker(&tick(&mut st, t1 + HISTORY_FREE_AFTER)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_reload_of_a_freed_history_is_shown_and_tried_again() {
    let n = HISTORY_FREE_MIN_COMMITS;
    let dir = long_repo("parked-free-failed", n);
    let mut st = parked_behind_new_tab(&dir, n, 0, 0.0);
    let t0 = Instant::now();
    tick(&mut st, t0);
    tick(&mut st, t0 + HISTORY_FREE_AFTER);
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(st.loading);

    // The folder went away while the tab was parked.
    let (generation, result) = (st.load_gen, Err("not found: could not find repository".to_owned()));
    update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Refresh, result }), Instant::now());
    assert!(!st.loading, "nothing loads, so the graph shows the failure rather than a spinner");
    assert_eq!(st.session.tab_view.history_freed, Some(n), "still a stub");
    assert_eq!(st.session.tab_view.history_reload_error.as_deref(), Some("not found: could not find repository"));
    assert_eq!(st.tabs.len(), 2, "the tab stays open");

    // Try again (F5) reloads in full; once it lands the error is gone.
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::Load(crate::effect::LoadEffect::Repo { known: None, prior: None, kind: LoadKind::Refresh, .. }))),
        "{fx:?}"
    );
    assert!(st.loading);
    let (generation, result) = (st.load_gen, Ok(Box::new(load(&dir, n))));
    update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Refresh, result }), Instant::now());
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), n);
    assert_eq!((st.session.tab_view.history_freed, st.session.tab_view.history_reload_error.as_deref()), (None, None));
    let _ = std::fs::remove_dir_all(&dir);
}
