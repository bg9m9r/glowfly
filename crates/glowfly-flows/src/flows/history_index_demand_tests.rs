//! The whole-history index of a very large history (`ON_DEMAND_MIN_COMMITS` commits by git's
//! commit-graph count) is built on the first search, not on open: the count's reading, the
//! threshold, what releases the build, updates, freeing and a parked tab asking for the count again.

use super::history_index_tests::{builds, opened_long_waiting, past_the_delay, run_build};
use super::history_index_wait_tests::index_of;
use super::tests::{drive_seeing, finish, finish_keeping, git, long_repo, state};
use super::update;
use glowfly_model::effect::{Effect, IndexEffect};
use glowfly_model::msg::{Msg, RepoMsg, SearchMsg, TabsMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::history_index::{
    BuildDelay, FIRST_BUILD_DELAY, HistoryCount, ON_DEMAND_MIN_COMMITS, PARKED_FREE_AFTER, SizeRead,
};
use std::path::PathBuf;
use std::time::Instant;

fn counted(st: &mut AppState, estimate: usize) -> Vec<Effect> {
    update(st, Msg::Search(SearchMsg::IndexCounted { estimate: Some(estimate) }), Instant::now())
}

fn query(st: &mut AppState, q: &str) -> Vec<Effect> {
    st.view.show_search = true;
    st.view.tab.search = q.into();
    update(st, Msg::Search(SearchMsg::QueryChanged), Instant::now())
}

fn tick(st: &mut AppState, t: Instant) -> Vec<Effect> {
    update(st, Msg::Tick(t), Instant::now())
}

/// A truncated history whose count says it is very large, with nothing searched yet.
fn opened_very_large(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir, _) = opened_long_waiting(name);
    let fx = counted(&mut st, ON_DEMAND_MIN_COMMITS);
    assert!(builds(&fx).is_empty(), "{fx:?}");
    (st, dir)
}

/// The query "needle" typed into a very large history, and its build run to the end.
fn searched_and_built(st: &mut AppState) {
    let fx = query(st, "needle");
    run_build(st, fx);
    assert!(st.session.search.index.ready.is_some());
}

#[test]
fn a_history_below_the_threshold_builds_after_the_delay() {
    let (mut st, dir, _) = opened_long_waiting("index-demand-below");
    let fx = counted(&mut st, ON_DEMAND_MIN_COMMITS - 1);
    assert!(builds(&fx).is_empty());
    assert_eq!(st.session.search.index.delay, BuildDelay::Wanted);
    let kept = past_the_delay(&mut st);
    let [IndexEffect::Build { base: None, .. }] = builds(&kept)[..] else { panic!("one build: {kept:?}") };
    assert!(st.session.search.index.building.as_ref().is_some_and(|b| b.fresh));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_very_large_history_waits_for_the_first_search() {
    let (mut st, dir) = opened_very_large("index-demand-wait");
    assert_eq!(st.session.search.index.delay, BuildDelay::Idle);
    let t0 = Instant::now();
    assert_eq!(st.index_wake(t0), None);
    for t in [t0, t0 + FIRST_BUILD_DELAY, t0 + 10 * FIRST_BUILD_DELAY] {
        assert!(builds(&tick(&mut st, t)).is_empty());
    }
    assert_eq!(st.history_count(), HistoryCount::Loaded);
    assert_eq!(st.indexing(), None);
    assert!(st.session.search.index.building.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_first_search_starts_a_very_large_build_at_once() {
    let (mut st, dir) = opened_very_large("index-demand-search");
    let fx = query(&mut st, "needle");
    let [IndexEffect::Build { base: None, .. }] = builds(&fx)[..] else { panic!("one build with no tick: {fx:?}") };
    assert!(st.session.search.index.building.as_ref().is_some_and(|b| b.fresh));
    assert_eq!(st.history_count(), HistoryCount::Indexing);
    assert_eq!(st.session.search.count_text(None, st.history_count()).as_deref(), Some("Indexing history…"));
    assert_eq!(st.indexing(), Some((0, None)));
    let fx = run_build(&mut st, fx);
    assert!(st.session.search.index.ready.is_some());
    assert!(fx.iter().any(|e| matches!(e, Effect::Index(IndexEffect::Query { .. }))), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_only_query_does_not_start_it() {
    let (mut st, dir) = opened_very_large("index-demand-path");
    let fx = query(&mut st, "path:f.txt");
    assert!(builds(&fx).is_empty(), "{fx:?}");
    assert!(!st.session.search.index.demanded);
    let fx = query(&mut st, "needle");
    assert_eq!(builds(&fx).len(), 1, "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_size_keeps_todays_behaviour() {
    let dir = long_repo("index-demand-unknown", 620);
    let mut st = state();
    st.settings.initial_commits = 500;
    let seen = |m: &Msg| matches!(m, Msg::Scoped { msg, .. } if matches!(**msg, Msg::Search(SearchMsg::IndexCounted { .. })));
    assert!(drive_seeing(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), seen), "the size is read on open");
    assert_eq!(st.session.search.index.size, SizeRead::Read(None), "no commit-graph");
    assert_eq!(st.session.search.index.delay, BuildDelay::Wanted);
    assert!(!drive_seeing(&mut st, Msg::Repo(RepoMsg::Refresh), seen), "read once per open");
    let kept = past_the_delay(&mut st);
    let [IndexEffect::Build { base: None, .. }] = builds(&kept)[..] else { panic!("one build: {kept:?}") };
    assert!(st.session.search.index.building.as_ref().is_some_and(|b| b.fresh));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_count_comes_from_the_commit_graph() {
    let dir = long_repo("index-count", 620);
    git(&dir, &["commit-graph", "write", "--reachable"]);
    let mut st = state();
    st.settings.initial_commits = 500;
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    finish_keeping(&mut st, fx);
    assert_eq!(st.session.search.index.size, SizeRead::Read(Some(620)));
    let kept = past_the_delay(&mut st);
    assert_eq!(builds(&kept).len(), 1, "below the threshold: {kept:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn search_before_the_count_then_a_large_count_builds_at_once() {
    let (mut st, dir, _) = opened_long_waiting("index-demand-early");
    st.session.search.index.size = SizeRead::Reading;
    let fx = query(&mut st, "needle");
    assert!(builds(&fx).is_empty(), "the delay path: {fx:?}");
    let fx = counted(&mut st, ON_DEMAND_MIN_COMMITS);
    assert_eq!(builds(&fx).len(), 1, "{fx:?}");
    assert_eq!(st.session.search.index.delay, BuildDelay::Passed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_update_above_the_threshold_does_not_wait_for_search() {
    let (mut st, dir) = opened_very_large("index-demand-update");
    searched_and_built(&mut st);
    update(&mut st, Msg::Search(SearchMsg::Close), Instant::now());
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "newer"]);
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    let [IndexEffect::Build { base: Some(_), .. }] = builds(&kept)[..] else { panic!("one update: {kept:?}") };
    let _ = std::fs::remove_dir_all(&dir);
}

/// A very large history searched and built (the query cleared unless `keep_query`), then parked
/// long enough to free its index; the tab is not yet shown again. Returns the time the freeing tick had.
fn freed_while_parked(st: &mut AppState, keep_query: bool) -> Instant {
    searched_and_built(st);
    if !keep_query {
        update(st, Msg::Search(SearchMsg::Close), Instant::now());
    }
    let key = glowfly_core::commit_index::IndexKey::of(st.snap.as_ref().unwrap());
    st.session.search.index.ready = Some(index_of(key, glowfly_model::state::history_index::FREE_MIN_COMMITS));
    let s = Instant::now();
    tick(st, s);
    let fx = update(st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    finish(st, fx);
    assert_eq!(st.active_tab, 1);
    let at = s + PARKED_FREE_AFTER;
    tick(st, at);
    let ix = &st.tabs[0].parked.as_ref().unwrap().session.search.index;
    assert!(ix.ready.is_none() && !ix.demanded, "freed, and search is no longer in use");
    at
}

#[test]
fn a_freed_very_large_index_waits_for_the_next_search() {
    let (mut st, dir) = opened_very_large("index-demand-freed");
    // The bar is cleared before parking.
    let at = freed_while_parked(&mut st, false);
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let mut kept = finish_keeping(&mut st, fx);
    for t in [at, at + FIRST_BUILD_DELAY, at + 10 * FIRST_BUILD_DELAY] {
        let fx = tick(&mut st, t);
        kept.extend(finish_keeping(&mut st, fx));
    }
    assert!(builds(&kept).is_empty(), "{kept:?}");
    let fx = query(&mut st, "needle");
    let [IndexEffect::Build { base: None, .. }] = builds(&fx)[..] else { panic!("one build: {fx:?}") };
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_freed_very_large_index_reloads_at_once_when_a_query_is_left() {
    let (mut st, dir) = opened_very_large("index-demand-left");
    freed_while_parked(&mut st, true);
    assert!(!st.tabs[0].parked.as_ref().unwrap().session.search.query.is_empty(), "the query is still in the box");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    assert_eq!(builds(&kept).len(), 1, "search is in use: {kept:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_count_dropped_while_parked_is_asked_again() {
    let (mut st, dir, _) = opened_long_waiting("index-demand-dropped");
    st.session.search.index.size = SizeRead::Reading;
    let fx = update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    finish(&mut st, fx);
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Index(IndexEffect::Count { .. }))), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
