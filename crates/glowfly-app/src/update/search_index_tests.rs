//! Searching with the whole-history index on `long_repo`'s 620 commits, 500 loaded: the doubling
//! fallback while it builds, one load straight to the matches once it answered (or none), Enter
//! past the loaded matches, a match past the graph's cap, path terms, stale answers and the end
//! of a filtered list.

use super::history_index_tests::{opened_long, run_build};
use super::tests::finish;
use super::update;
use crate::effect::{Effect, IndexEffect, LoadKind};
use crate::msg::{LoadMsg, Msg, SearchMsg};
use crate::state::history_index::{HistoryCount, IndexAnswer};
use crate::state::load_more::Reach;
use crate::state::{AppState, Selection};
use glowfly_core::commit_index::{CommitIndex, IndexKey};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn send(st: &mut AppState, msg: SearchMsg) -> Vec<Effect> {
    update(st, Msg::Search(msg), Instant::now())
}

/// The bar shown with `q` typed into it, filtering when `filter`.
fn search(st: &mut AppState, q: &str, filter: bool) -> Vec<Effect> {
    st.view.show_search = true;
    st.view.search = q.into();
    let mut fx = Vec::new();
    if st.search.filter != filter {
        fx.extend(send(st, SearchMsg::ToggleFilter));
    }
    fx.extend(send(st, SearchMsg::QueryChanged));
    fx
}

fn loads(fx: &[Effect]) -> Vec<usize> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LoadRepo { opts, kind: LoadKind::Refresh, .. } => Some(opts.max_commits),
            _ => None,
        })
        .collect()
}

fn queries(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Index(IndexEffect::Query { .. }))).count()
}

/// Runs the effects in `fx` other than loads and builds (the index query among them) one round:
/// the effects their results raised.
fn answer(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let mut out = Vec::new();
    for e in fx {
        if matches!(e, Effect::Index(IndexEffect::Query { .. })) {
            let msg = crate::runner::execute(e).unwrap();
            out.extend(update(st, msg, Instant::now()));
        }
    }
    out
}

/// `opened_long` with its index built.
fn indexed(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir, kept) = opened_long(name);
    let fx = run_build(&mut st, kept);
    finish(&mut st, fx);
    assert!(st.search.index.ready.is_some());
    (st, dir)
}

fn summary(st: &AppState) -> Option<String> {
    let Selection::Commit(oid) = st.selection else { return None };
    let snap = st.snap.as_ref()?;
    Some(snap.commits[*snap.row_of.get(&oid)?].summary.clone())
}

#[test]
fn while_indexing_the_filter_falls_back_to_doubling_loads() {
    let (mut st, dir, _) = opened_long("index-fallback");
    let fx = search(&mut st, "needle", true);
    assert_eq!(loads(&fx), [1000], "one doubling batch, as without an index");
    assert_eq!(queries(&fx), 0);
    assert_eq!(st.search.count_text(None, st.history_count()).as_deref(), Some("Indexing history…"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_an_index_the_filter_loads_straight_to_the_matches() {
    let (mut st, dir) = indexed("index-straight");
    let fx = search(&mut st, "needle", true);
    assert!(loads(&fx).is_empty(), "waits for the index's answer: {fx:?}");
    assert_eq!(queries(&fx), 1);
    let fx = answer(&mut st, fx);
    assert_eq!(loads(&fx), [1000], "through the oldest needle (620), rounded up to a whole step");
    finish(&mut st, fx);
    assert!(!st.loading);
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), 620);
    assert_eq!(st.search.matches.len(), 3, "the three needles are in");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_match_anywhere_needs_no_load() {
    let (mut st, dir) = indexed("index-nothing");
    let fx = search(&mut st, "zzz", true);
    let fx = answer(&mut st, fx);
    assert!(loads(&fx).is_empty(), "{fx:?}");
    assert_eq!(st.search_reach(), Reach::Whole { shallow: false, commits: 620 });
    assert_eq!(st.search.count_text(None, st.history_count()).as_deref(), Some("No matches"));
    // Keep searching has nothing to load either.
    assert!(loads(&update(&mut st, Msg::Load(LoadMsg::SearchFurther), Instant::now())).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn enter_past_the_loaded_matches_loads_and_selects_the_next() {
    let (mut st, dir) = indexed("index-enter");
    let fx = search(&mut st, "beacon", false);
    finish(&mut st, fx);
    assert_eq!(st.history_count(), HistoryCount::Whole { positions: &[0, 619], commits: 620, complete: true });
    assert_eq!(st.search.matches, [0]);

    let fx = send(&mut st, SearchMsg::Next);
    finish(&mut st, fx);
    assert_eq!(summary(&st).as_deref(), Some("commit 619 beacon"), "the newest match first");
    assert_eq!(st.search.count_text(Some(0), st.history_count()).as_deref(), Some("1 of 2"));

    let fx = send(&mut st, SearchMsg::Next);
    assert_eq!(loads(&fx), [1000], "loads through the oldest commit");
    assert!(st.search.index.jump.is_some());
    finish(&mut st, fx);
    assert_eq!(summary(&st).as_deref(), Some("commit 0 needle beacon"), "selected once loaded");
    assert_eq!(st.search.index.jump, None);

    let fx = send(&mut st, SearchMsg::Next);
    finish(&mut st, fx);
    assert_eq!(summary(&st).as_deref(), Some("commit 619 beacon"), "wraps to the newest");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stages an index answer for the current query with these walk positions.
fn staged(st: &mut AppState, positions: Vec<u32>) {
    let snap = st.snap.as_ref().unwrap();
    let index = Arc::new(CommitIndex::from_commits(IndexKey::of(snap), &snap.commits));
    st.search.index.ready = Some(Arc::clone(&index));
    st.search.index.pending = None;
    st.search.index.answer = Some(IndexAnswer { query: st.search.query.clone(), index, positions });
}

#[test]
fn a_match_past_the_graph_cap_is_logged_not_loaded() {
    let (mut st, dir, _) = opened_long("index-cap");
    search(&mut st, "zzz", false);
    staged(&mut st, vec![250_000]);
    let fx = send(&mut st, SearchMsg::Next);
    assert!(loads(&fx).is_empty(), "{fx:?}");
    assert!(st.log.iter().any(|l| l.text == "The next match is older than the newest 200000 commits the graph can show"));
    assert!(st.search.index.jump.is_none());
    st.search.filter = true;
    assert_eq!(st.search_reach(), Reach::PastCap { matches: 1 });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn path_terms_never_ask_the_index() {
    let (mut st, dir) = indexed("index-paths");
    let fx = search(&mut st, "path:a.txt fix", false);
    assert_eq!(queries(&fx), 0, "{fx:?}");
    assert_eq!(st.history_count(), HistoryCount::Loaded);
    assert!(st.current_index().is_none());
    finish(&mut st, fx);
    assert_eq!(st.search.count_text(None, st.history_count()).as_deref(), Some("No matches"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_query_answer_is_ignored() {
    let (mut st, dir) = indexed("index-stale-query");
    let first = search(&mut st, "needle", false);
    let first_request = st.search.index.pending.unwrap();
    let second = search(&mut st, "beacon", false);
    assert_eq!((queries(&first), queries(&second)), (1, 1));
    // The first query's answer arrives after the second was asked.
    update(&mut st, Msg::Search(SearchMsg::IndexQueried { request: first_request, result: Ok(vec![617, 618, 619]) }), Instant::now());
    assert!(st.search.index.answer.is_none() && st.search.index.pending.is_some());
    answer(&mut st, second);
    assert_eq!(st.index_answer().map(|a| a.positions.clone()), Some(vec![0, 619]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn near_history_end_while_filtering_loads_the_next_screen_of_matches_or_nothing() {
    let (mut st, dir) = indexed("index-near-end");
    st.view.graph_fit_rows = 10;
    let fx = search(&mut st, "commit", true);
    let fx = answer(&mut st, fx);
    assert!(loads(&fx).is_empty(), "500 loaded matches fill the graph");
    let fx = update(&mut st, Msg::Load(LoadMsg::NearHistoryEnd), Instant::now());
    assert_eq!(loads(&fx), [1000], "through the next ten matches");
    finish(&mut st, fx);

    let (mut st, dir2) = indexed("index-near-end-none");
    st.view.graph_fit_rows = 10;
    let fx = search(&mut st, "619 beacon", true);
    finish(&mut st, fx);
    assert_eq!(st.search.matches.len(), 1);
    let fx = update(&mut st, Msg::Load(LoadMsg::NearHistoryEnd), Instant::now());
    assert!(loads(&fx).is_empty(), "no older match: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}
