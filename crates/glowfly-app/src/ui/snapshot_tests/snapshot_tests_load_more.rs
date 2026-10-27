//! The graph of a history longer than the commit limit: the end row's count and Load more,
//! asking for more near the end, and the row shown instead while more loads or after loading it
//! failed.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{LoadMsg, Msg, SearchMsg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn graph_end_row_offers_more_commits() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "load-more");
    st.snap.as_mut().unwrap().truncated = true;
    let last = st.snap.as_ref().unwrap().commits.last().unwrap().summary.clone();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_load_more");
    let n = st_commits(&h);
    let sentence = format!("Showing the newest {n} commits.");
    let end = h.get_by_label(&sentence).bounding_box().expect("the end row's sentence");
    let button = h.get_by_label("Load more").bounding_box().unwrap();
    let row = h.get_by_label(&last).bounding_box().unwrap();
    assert!(end.y0 >= row.y1 - 0.5, "the end row is after the last commit: {end:?} {row:?}");
    assert!(button.y0 >= end.y0 && button.y1 <= end.y1 && button.x0 > end.x0 + end.width() / 2.0, "Load more at the end row's right");
    h.get_by_label("Load more").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Load(LoadMsg::LoadMoreCommits))));
    drop(h);

    // At the cap only the sentence shows.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "load-more");
    let snap = st.snap.as_mut().unwrap();
    snap.truncated = true;
    snap.loaded_with.max_commits = crate::state::MAX_COMMITS;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&sentence).is_some());
    assert!(h.query_by_label("Load more").is_none(), "nothing more can load");
    drop(h);
    remove_fixture(&dir);
}

/// How many commits the harness's snapshot holds.
fn st_commits(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>) -> usize {
    h.state().0.snap.as_ref().map_or(0, |s| s.commits.len())
}

/// The fixture as a history cut at the commit limit: `truncated`, loaded with exactly the limit.
fn truncated(name: &str) -> (AppState, PathBuf) {
    truncated_in(ThemeChoice::Dark, name)
}

fn truncated_in(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let limit = st.settings.commit_limit();
    let snap = st.snap.as_mut().unwrap();
    snap.truncated = true;
    snap.loaded_with.max_commits = limit;
    (st, dir)
}

fn asks_for_more(msgs: &[Msg]) -> bool {
    msgs.iter().any(|m| matches!(m, Msg::Load(LoadMsg::NearHistoryEnd)))
}

#[test]
fn graph_loading_more() {
    let (mut st, dir) = truncated("loading-more");
    st.refresh.extra_commits = st.settings.commit_limit();
    st.loading = true;
    // The spinner keeps repainting; a fixed number of frames keeps the image reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("graph_loading_more");
    let _ = h.get_by_label("Loading more commits…");
    assert!(h.query_by_label("Load more").is_none(), "the spinner row replaces the end row");
    assert!(!asks_for_more(&h.state().1), "a load for more is running");
    remove_fixture(&dir);
}

#[test]
fn graph_near_the_end_asks_for_more() {
    let (st, dir) = truncated("near-end");
    let mut h = harness(st, true);
    h.run();
    assert!(asks_for_more(&h.state().1), "every row is visible, so the end is near");

    let (mut st, _) = truncated("near-end");
    st.settings.load_more_on_scroll = false;
    let mut h = harness(st, true);
    h.run();
    assert!(!asks_for_more(&h.state().1), "the preference is off");

    // A filtering search never asks from the view: with too few matches, update loads further
    // back by itself (search_deeper.rs), and the row under the matches spins meanwhile.
    let (mut st, _) = truncated("near-end");
    st.view.show_search = true;
    st.session.tab_view.search = "page".into();
    crate::update::update(&mut st, Msg::Search(SearchMsg::QueryChanged), Instant::now());
    crate::update::update(&mut st, Msg::Search(SearchMsg::ToggleFilter), Instant::now());
    assert!(st.session.search.filter && !st.session.search.matches.is_empty());
    assert!(st.searching_older());
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(!asks_for_more(&h.state().1), "the search filters rows");

    let (mut st, _) = truncated("near-end");
    st.snap.as_mut().unwrap().truncated = false;
    let mut h = harness(st, true);
    h.run();
    assert!(!asks_for_more(&h.state().1), "the whole history is loaded");
    remove_fixture(&dir);
}

#[test]
fn graph_load_more_failed() {
    for (theme, name, dark, retrying) in
        [(ThemeChoice::Dark, "graph_load_more_failed", true, true), (ThemeChoice::Light, "graph_load_more_failed_light", false, false)]
    {
        let (mut st, dir) = truncated_in(theme, &format!("more-failed-{dark}"));
        st.refresh.extra_commits = st.settings.commit_limit();
        st.loading = false;
        st.refresh.more_failed = Some("object not found - no match for id (1234abcd)".into());
        // A retry still waiting, in the dark image; used up in the light one.
        st.refresh.more_retry_at = retrying.then(|| Instant::now() + Duration::from_secs(3));
        let mut h = harness(st, dark);
        h.run();
        h.check_snapshot(name);
        assert!(h.query_by_label_contains("Couldn't load more commits").is_some());
        assert_eq!(h.query_by_label("Trying again shortly…").is_some(), retrying);
        assert!(!asks_for_more(&h.state().1), "the failed load is held, not asked for again");
        h.state_mut().1.clear();
        h.get_by_label("Try again").click();
        h.run();
        assert!(h.state().1.iter().any(|m| matches!(m, Msg::Load(LoadMsg::RetryLoadMore))));
        drop(h);
        remove_fixture(&dir);
    }
}
