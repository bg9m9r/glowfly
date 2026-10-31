//! A filtering search in a history cut at the commit limit, loading further back: the row under
//! a short list of matches while older commits are searched, and the no-matches pane while
//! searching, after the batch budget is spent (Keep searching) and after a load failed.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_search::{searching, send, with_two_authors};
use crate::msg::{LoadMsg, Msg, SearchMsg};
use crate::state::AppState;
use crate::state::load_more::{DEEPER_BATCHES, DeeperSearch};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The two-author fixture as a history cut at the commit limit, searched for `q` with the
/// filter on: the search starts loading older commits by itself.
fn filtering(name: &str, q: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, name);
    let limit = st.settings.commit_limit();
    let snap = st.snap.as_mut().unwrap();
    snap.truncated = true;
    snap.loaded_with.max_commits = limit;
    searching(&mut st, q);
    send(&mut st, SearchMsg::ToggleFilter);
    assert!(st.searching_older(), "the search loads further back");
    (st, dir)
}

fn commits(st: &AppState) -> usize {
    st.snap.as_ref().unwrap().commits.len()
}

/// Clicks the button labelled `label` and returns what it pushed.
fn click(h: &mut H, label: &str) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn graph_searching_older() {
    let (st, dir) = filtering("search-older-row", "release notes");
    assert_eq!(st.session.search.matches.len(), 1);
    // The spinner keeps repainting; a fixed number of frames keeps the image reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("graph_searching_older");
    assert!(h.query_by_label("Searching older commits…").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_searching_older_empty() {
    let (st, dir) = filtering("search-older-empty", "nothing like this");
    let n = commits(&st);
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("graph_search_searching_older_empty");
    assert!(h.query_by_label(&format!("No matches in the newest {n} commits. Searching older ones…")).is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_keep_searching() {
    let (mut st, dir) = filtering("search-keep", "nothing like this");
    // The batch budget spent, with nothing found.
    st.loading = false;
    st.refresh.extra_commits = 0;
    st.refresh.deeper = Some(DeeperSearch { query: st.session.search.query.clone(), batches: DEEPER_BATCHES });
    let n = commits(&st);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_search_keep_searching");
    assert!(h.query_by_label_contains(&format!("Searched the newest {n} commits.")).is_some());
    let msgs = click(&mut h, "Keep searching");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Load(LoadMsg::SearchFurther))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_load_failed() {
    let (mut st, dir) = filtering("search-load-failed", "nothing like this");
    // The batch failed: the snapshot is the same, more is still wanted.
    st.loading = false;
    st.refresh.more_failed = Some("object not found - no match for id (1234abcd)".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_search_load_failed");
    assert!(h.query_by_label("Couldn't load more commits").is_some());
    assert!(h.query_by_label("Copy details").is_some());
    let msgs = click(&mut h, "Try again");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Load(LoadMsg::RetryLoadMore))));
    drop(h);
    remove_fixture(&dir);
}
