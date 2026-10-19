//! The graph of a history longer than the commit limit: the header's count and Load more link,
//! asking for more near the end, and the row shown while more loads.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::{LoadMsg, Msg, SearchMsg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::settings::ThemeChoice;

#[test]
fn graph_header_offers_more_commits() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "load-more");
    st.snap.as_mut().unwrap().truncated = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_load_more");
    h.get_by_label("Load more").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Load(LoadMsg::LoadMoreCommits))));

    // With room in the message column, the count shows too.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "load-more");
    st.snap.as_mut().unwrap().truncated = true;
    st.settings.columns.commit = false;
    st.settings.columns.date = false;
    render("graph_load_more_count", st, true);
    remove_fixture(&dir);
}

/// The fixture as a history cut at the commit limit: `truncated`, loaded with exactly the limit.
fn truncated(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
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

    // A filtering search never loads more by itself.
    let (mut st, _) = truncated("near-end");
    st.view.show_search = true;
    st.view.search = "page".into();
    crate::update::update(&mut st, Msg::Search(SearchMsg::QueryChanged), Instant::now());
    crate::update::update(&mut st, Msg::Search(SearchMsg::ToggleFilter), Instant::now());
    assert!(st.search.filter && !st.search.matches.is_empty());
    let mut h = harness(st, true);
    h.run();
    assert!(!asks_for_more(&h.state().1), "the search filters rows");

    let (mut st, _) = truncated("near-end");
    st.snap.as_mut().unwrap().truncated = false;
    let mut h = harness(st, true);
    h.run();
    assert!(!asks_for_more(&h.state().1), "the whole history is loaded");
    remove_fixture(&dir);
}
