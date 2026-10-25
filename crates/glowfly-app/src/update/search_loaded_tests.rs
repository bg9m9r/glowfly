//! Headless tests for searching a large loaded history: typing waits for a pause, the match runs
//! on the query worker with the same matches as on the UI thread, and older answers are ignored.
//! Also the large-history fixture the graph's off-thread tests share.

use super::search_loaded::TYPING_PAUSE;
use super::tests::{finish, state};
use super::update;
use crate::effect::{Effect, IndexEffect};
use crate::msg::{Msg, SearchMsg};
use crate::state::refresh::LARGE_HISTORY;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::repo::CommitInfo;
use std::collections::HashSet;
use std::time::Instant;

/// The oid of synthetic commit `i`.
pub(super) fn oid_of(i: usize) -> Oid {
    let mut b = [0u8; 20];
    b[..8].copy_from_slice(&(i as u64 + 1).to_be_bytes());
    Oid::from_bytes(&b).unwrap()
}

/// `n` synthetic commits, newest first: commit `i` says "commit {i}" ("needle" too every 997th,
/// "Überblick" every 1999th), by Ada or José, and every 7th merges the commit three below it.
pub(super) fn history(n: usize) -> Vec<CommitInfo> {
    (0..n)
        .map(|i| {
            let mut summary = format!("commit {i}");
            if i % 997 == 0 {
                summary.push_str(" needle");
            }
            if i % 1999 == 0 {
                summary.push_str(" Überblick");
            }
            let (author, email) = if i % 3 == 0 { ("José Müller", "jose@example.org") } else { ("Ada Lovelace", "ada@example.com") };
            let mut parents: Vec<Oid> = (i + 1 < n).then(|| oid_of(i + 1)).into_iter().collect();
            if i % 7 == 0 && i + 3 < n {
                parents.push(oid_of(i + 3));
            }
            CommitInfo {
                oid: oid_of(i),
                summary: summary.into(),
                body: if i % 2 == 0 { format!("Body of {i}").into() } else { Default::default() },
                author: glowfly_core::repo::Author::new(author, email),
                time: 1_600_000_000 - i as i64,
                parents: parents.into(),
            }
        })
        .collect()
}

/// A state showing `n` synthetic commits (HEAD at the newest, on main) with the search bar open.
/// The one-commit repository behind it is in the returned folder.
pub(super) fn large_state(name: &str, n: usize) -> (AppState, std::path::PathBuf) {
    let mut snap = crate::state::op_fixture::snapshot(name);
    let dir = snap.workdir.clone();
    snap.commits = history(n).into();
    snap.row_of = snap.commits.iter().enumerate().map(|(i, c)| (c.oid, i)).collect();
    snap.head_oid = Some(oid_of(0));
    snap.refs.clear();
    for b in &mut snap.locals {
        b.oid = Some(oid_of(0));
    }
    snap.unstaged.clear();
    snap.staged.clear();
    snap.lay_out_graph();
    let mut st = state();
    st.snap = Some(snap);
    st.view.show_search = true;
    (st, dir)
}

/// Types `q` into the search field at `now`.
fn type_query(st: &mut AppState, q: &str, now: Instant) -> Vec<Effect> {
    st.view.search = q.to_owned();
    update(st, Msg::Search(SearchMsg::QueryChanged), now)
}

/// The requests of the loaded-history matches in `fx`.
fn asked(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Index(IndexEffect::MatchLoaded { request, .. }) => Some(*request),
            _ => None,
        })
        .collect()
}

/// The matches of `q` on the UI thread.
fn expected(st: &AppState, q: &str) -> Vec<usize> {
    CommitQuery::parse(q).matching_rows(&st.snap.as_ref().unwrap().commits, &HashSet::new())
}

#[test]
fn a_large_history_is_matched_after_a_pause_on_the_worker() {
    let (mut st, dir) = large_state("loaded-pause", LARGE_HISTORY + 10);
    let t0 = Instant::now();
    let fx = type_query(&mut st, "nee", t0);
    assert!(asked(&fx).is_empty() && st.search.matches.is_empty(), "nothing matched while typing");
    let fx = type_query(&mut st, "needle", t0 + TYPING_PAUSE / 2);
    assert!(asked(&fx).is_empty(), "the next keystroke waits again");
    let fx = update(&mut st, Msg::Tick(t0 + TYPING_PAUSE), Instant::now());
    assert!(asked(&fx).is_empty(), "the pause after the last keystroke isn't over");
    let fx = update(&mut st, Msg::Tick(t0 + TYPING_PAUSE * 2), Instant::now());
    assert_eq!(asked(&fx).len(), 1, "one match for the query typed: {fx:?}");
    assert!(st.search.matches.is_empty(), "applied when the answer comes");
    finish(&mut st, fx);
    assert_eq!(st.search.matches, expected(&st, "needle"));
    for q in ["needle", "überblick", "author:josé needle", "commit 2000", "body of 4", "zzz"] {
        let want = expected(&st, q);
        type_query(&mut st, q, t0);
        let fx = update(&mut st, Msg::Tick(t0 + TYPING_PAUSE), Instant::now());
        finish(&mut st, fx);
        assert_eq!(st.search.matches, want, "{q}: the same matches as on the UI thread");
    }
    assert_eq!(expected(&st, "needle").len(), (LARGE_HISTORY + 10).div_ceil(997));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_older_answer_is_ignored() {
    let (mut st, dir) = large_state("loaded-stale", LARGE_HISTORY);
    let t0 = Instant::now();
    type_query(&mut st, "needle", t0);
    let first = asked(&update(&mut st, Msg::Tick(t0 + TYPING_PAUSE), Instant::now()));
    type_query(&mut st, "commit 1", t0);
    let second = asked(&update(&mut st, Msg::Tick(t0 + TYPING_PAUSE), Instant::now()));
    let (&[first], &[second]) = (&first[..], &second[..]) else { panic!("one request each") };
    assert!(second > first);
    let answer = |request, matches: Vec<usize>| Msg::Search(SearchMsg::LoadedMatched { request, result: Ok(matches) });
    update(&mut st, answer(first, vec![0, 997]), Instant::now());
    assert!(st.search.matches.is_empty(), "the replaced request's answer is dropped");
    update(&mut st, answer(second, vec![1, 10]), Instant::now());
    assert_eq!(st.search.matches, [1, 10]);
    update(&mut st, answer(second, vec![5]), Instant::now());
    assert_eq!(st.search.matches, [1, 10], "answered once");

    // A new history (or any match on the UI thread) drops the request in flight.
    type_query(&mut st, "needle", t0);
    let third = asked(&update(&mut st, Msg::Tick(t0 + TYPING_PAUSE), Instant::now()));
    super::search::recompute(&mut st);
    let synced = st.search.matches.clone();
    assert_eq!(synced, expected(&st, "needle"));
    update(&mut st, answer(third[0], vec![3]), Instant::now());
    assert_eq!(st.search.matches, synced, "an answer for the history before is dropped");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn enter_matches_a_waiting_query_first() {
    let (mut st, dir) = large_state("loaded-enter", LARGE_HISTORY);
    let t0 = Instant::now();
    type_query(&mut st, "needle", t0);
    let fx = update(&mut st, Msg::Search(SearchMsg::Next), t0);
    assert!(asked(&fx).is_empty(), "matched at once, not on the worker");
    assert_eq!(st.search.matches, expected(&st, "needle"));
    assert_eq!(st.selection, Selection::Commit(oid_of(0)), "the newest match");
    assert_eq!(st.search.loaded.due, None);
    // A late tick has nothing left to do.
    assert!(asked(&update(&mut st, Msg::Tick(t0 + TYPING_PAUSE), Instant::now())).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_small_history_is_matched_at_once() {
    let (mut st, dir) = large_state("loaded-small", 50);
    let fx = type_query(&mut st, "commit 4", Instant::now());
    assert!(asked(&fx).is_empty() && st.search.loaded.due.is_none());
    assert_eq!(st.search.matches, expected(&st, "commit 4"));
    let _ = std::fs::remove_dir_all(&dir);
}
