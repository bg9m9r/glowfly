//! The whole-history search index in the views: the search bar's count while it builds and once
//! it answered, the status bar's progress, and the no-matches pane after the index searched
//! everything or found matches only past the graph's cap. Indexes are staged, never built.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_search::{searching, send, with_two_authors};
use crate::msg::SearchMsg;
use crate::state::history_index::{Building, IndexAnswer};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::PathBuf;
use std::sync::Arc;
use trellis_core::commit_index::{CommitIndex, IndexKey};
use trellis_core::repo::CommitInfo;
use trellis_core::settings::ThemeChoice;

/// The two-author fixture with its history marked as cut short.
fn truncated(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, name);
    st.snap.as_mut().unwrap().truncated = true;
    (st, dir)
}

/// A fresh build of the fixture's index, `indexed` commits in, of about `estimate`.
fn building(st: &mut AppState, indexed: usize, estimate: Option<usize>) {
    let key = IndexKey::of(st.snap.as_ref().unwrap());
    st.search.index.building = Some(Building { build: 1, key, indexed, estimate, fresh: true });
}

/// An index of `commits` made-up commits for the fixture's history, answering the current query
/// with `positions`.
fn answered(st: &mut AppState, commits: usize, positions: Vec<u32>) {
    let info = |i: usize| {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(&(i as u64 + 1).to_be_bytes());
        CommitInfo {
            oid: Oid::from_bytes(&id).unwrap(),
            summary: format!("commit {i}"),
            body: String::new(),
            author: "A".into(),
            email: "a@example.com".into(),
            time: 0,
            parents: Vec::new(),
        }
    };
    let list: Vec<CommitInfo> = (0..commits).map(info).collect();
    let index = Arc::new(CommitIndex::from_commits(IndexKey::of(st.snap.as_ref().unwrap()), &list));
    st.search.index.ready = Some(Arc::clone(&index));
    st.search.index.answer = Some(IndexAnswer { query: st.search.query.clone(), index, positions });
}

#[test]
fn search_bar_indexing() {
    let (mut st, dir) = truncated("index-bar-building");
    building(&mut st, 0, None);
    searching(&mut st, "page");
    assert_eq!(st.search.matches.len(), 3);
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label("3 so far, indexing…").is_some());
    h.check_snapshot("search_bar_indexing");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn search_bar_whole_history_count() {
    let (mut st, dir) = truncated("index-bar-whole");
    searching(&mut st, "page");
    answered(&mut st, 812, (0..40).collect());
    let snap = st.snap.as_ref().unwrap();
    let second = snap.commits[st.search.matches[1]].oid;
    st.selection = Selection::Commit(second);
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label("2 of 40").is_some());
    h.check_snapshot("search_bar_whole_history_count");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn status_bar_indexing() {
    let (mut st, dir) = truncated("index-status");
    building(&mut st, 1200, Some(5000));
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label("Indexing history: 1200 of about 5000 commits").is_some());
    h.check_snapshot("status_bar_indexing");
    h.state_mut().0.search.index.building.as_mut().unwrap().estimate = None;
    h.run_steps(2);
    assert!(h.query_by_label("Indexing history: 1200 commits").is_some(), "without git's commit count");
    h.state_mut().0.search.index.building.as_mut().unwrap().fresh = false;
    h.run_steps(2);
    assert!(h.query_by_label_contains("Indexing history").is_none(), "an update runs silently");
    drop(h);
    remove_fixture(&dir);
}

/// The filter on with `q`, which matches nothing loaded.
fn filtering_nothing(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = truncated(name);
    searching(&mut st, "nothing like this");
    send(&mut st, SearchMsg::ToggleFilter);
    assert!(st.search.matches.is_empty());
    (st, dir)
}

#[test]
fn graph_search_empty_indexed_whole() {
    let (mut st, dir) = filtering_nothing("index-empty-whole");
    answered(&mut st, 812, Vec::new());
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("No matches").is_some());
    assert!(h.query_by_label_contains("Searched the whole history (812 commits).").is_some());
    assert!(h.query_by_label("Keep searching").is_none());
    assert!(h.query_by_label("Load more commits").is_none());
    h.check_snapshot("graph_search_empty_indexed_whole");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_empty_beyond_reach() {
    let (mut st, dir) = filtering_nothing("index-empty-beyond");
    answered(&mut st, 812, vec![250_000, 250_001, 250_002]);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("3 matches").is_some());
    assert!(h.query_by_label_contains("3 older commits match, past the newest 200000 the graph can show.").is_some());
    assert!(h.query_by_label("Keep searching").is_none());
    h.check_snapshot("graph_search_empty_beyond_reach");
    drop(h);
    remove_fixture(&dir);
}
