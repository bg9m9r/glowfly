//! Headless tests for keeping the graph's top visible commit in place across a reload
//! (`update/graph_anchor.rs`): new commits above move the scroll by their rows, a rewritten top
//! commit anchors on the next one still shown, a filtered graph counts only the matches, a
//! pending jump moves with the rows, and nothing moves at the top, on opening or when no rows
//! were added. The selection and the pane focus stay.

use super::tests::{git, long_repo, state};
use super::update;
use crate::effect::{Effect, LoadEffect, LoadKind};
use crate::msg::{LoadMsg, Msg, RepoMsg};
use crate::state::graph_rows::GraphRows;
use crate::state::layout::Pane;
use crate::state::{AppState, Selection, Tab};
use git2::Oid;
use glowfly_core::repo::{self, LoadOptions, Snapshot};
use std::path::{Path, PathBuf};
use std::time::Instant;

const ROW_H: f32 = 36.0;
/// Scrolled to row 10, with 9 px of it scrolled off.
const SCROLL: f32 = 10.0 * ROW_H + 9.0;

fn snapshot_of(dir: &Path, n: usize) -> Snapshot {
    repo::load(dir, &LoadOptions::all(n)).unwrap()
}

/// A state showing `long_repo(name, count)` loaded `n` deep, scrolled to `SCROLL` with commit
/// 14 (by row) selected and the graph focused.
fn scrolled(name: &str, count: usize, n: usize) -> (AppState, PathBuf) {
    let dir = long_repo(name, count);
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    let snap = snapshot_of(&dir, n);
    st.session.selection = Selection::Commit(snap.commits[14].oid);
    st.snap = Some(snap);
    st.view.tab.graph_scroll = (SCROLL, 400.0);
    st.view.tab.graph_row_h = ROW_H;
    st.view.pane_focus = Some(Pane::Central);
    (st, dir)
}

/// The runner's answer to the running load: `dir` loaded `n` deep, as `kind`.
fn reload_with(st: &mut AppState, dir: &Path, n: usize, kind: LoadKind) {
    let result = Ok(Box::new(snapshot_of(dir, n)));
    update(st, Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind, result }), Instant::now());
}

fn commit_empty(dir: &Path, msgs: &[&str]) {
    for m in msgs {
        git(dir, &["commit", "-q", "--allow-empty", "-m", m]);
    }
}

fn row_of(st: &AppState, oid: Oid) -> Option<usize> {
    GraphRows::new(st.snap.as_ref().unwrap(), &st.session.search, st.view.show_search).row_of(oid)
}

fn commit_at(st: &AppState, i: usize) -> Oid {
    st.snap.as_ref().unwrap().commits[i].oid
}

/// Checks the anchor wrote `off`, moving any pending row.
#[track_caller]
fn anchored_at(st: &AppState, off: f32) {
    assert_eq!(st.view.scroll_offset, Some(off));
    assert_eq!(st.view.scroll_to_row, None);
    assert_eq!(st.view.tab.graph_scroll.0, off);
}

#[test]
fn a_refresh_with_new_commits_above_keeps_the_top_commit_in_place() {
    let (mut st, dir) = scrolled("anchor-refresh", 60, 100);
    let (top, picked) = (commit_at(&st, 10), commit_at(&st, 14));
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert!(st.loading);
    commit_empty(&dir, &["on top 1", "on top 2", "on top 3"]);
    reload_with(&mut st, &dir, 100, LoadKind::Refresh);

    anchored_at(&st, 13.0 * ROW_H + 9.0);
    assert_eq!(row_of(&st, top), Some(13));
    assert_eq!(st.session.selection, Selection::Commit(picked));
    assert_eq!(row_of(&st, picked), Some(17));
    assert_eq!(st.view.pane_focus, Some(Pane::Central));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_load_more_turned_full_refresh_keeps_the_top_commit_in_place() {
    let (mut st, dir) = scrolled("anchor-more", 1200, 500);
    st.settings.initial_commits = 500;
    let picked = commit_at(&st, 14);
    let fx = update(&mut st, Msg::Load(LoadMsg::LoadMoreCommits), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Repo { kind: LoadKind::More, .. }))), "{fx:?}");
    commit_empty(&dir, &["new 1", "new 2"]);
    reload_with(&mut st, &dir, 1000, LoadKind::Refresh);

    anchored_at(&st, SCROLL + 2.0 * ROW_H);
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), 1000);
    assert!(!st.loading_more());
    assert_eq!(st.session.selection, Selection::Commit(picked));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rewritten_top_commit_anchors_on_the_next_one_still_shown() {
    let (mut st, dir) = scrolled("anchor-rewritten", 60, 100);
    let (top, next) = (commit_at(&st, 10), commit_at(&st, 11));
    let picked = commit_at(&st, 19);
    st.session.selection = Selection::Commit(picked);
    // Drops commits 49 to 59 (rows 0 to 10), then makes 14 new ones.
    git(&dir, &["reset", "-q", "--hard", "main~11"]);
    let new: Vec<String> = (0..14).map(|i| format!("new {i}")).collect();
    commit_empty(&dir, &new.iter().map(String::as_str).collect::<Vec<_>>());
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    reload_with(&mut st, &dir, 100, LoadKind::Refresh);

    assert_eq!(row_of(&st, top), None, "the old top commit is gone");
    assert_eq!(row_of(&st, next), Some(14));
    anchored_at(&st, SCROLL + 3.0 * ROW_H);
    assert_eq!(st.session.selection, Selection::Commit(picked));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn at_the_top_new_commits_show_above() {
    let (mut st, dir) = scrolled("anchor-top", 60, 100);
    let picked = commit_at(&st, 14);
    st.view.tab.graph_scroll = (0.0, 400.0);
    commit_empty(&dir, &["on top 1", "on top 2", "on top 3"]);
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    reload_with(&mut st, &dir, 100, LoadKind::Refresh);
    assert_eq!((st.view.scroll_offset, st.view.scroll_to_row), (None, None));
    assert_eq!(st.session.selection, Selection::Commit(picked));

    // A graph never drawn has no row height to anchor with.
    st.view.tab.graph_scroll = (SCROLL, 400.0);
    st.view.tab.graph_row_h = 0.0;
    commit_empty(&dir, &["on top 4"]);
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    reload_with(&mut st, &dir, 100, LoadKind::Refresh);
    assert_eq!((st.view.scroll_offset, st.view.scroll_to_row), (None, None));
    assert_eq!(st.session.selection, Selection::Commit(picked));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_moves_when_no_rows_were_added() {
    let (mut st, dir) = scrolled("anchor-same", 60, 100);
    let picked = commit_at(&st, 14);
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    reload_with(&mut st, &dir, 100, LoadKind::Refresh);
    assert_eq!((st.view.scroll_offset, st.view.scroll_to_row), (None, None));
    assert_eq!(st.view.tab.graph_scroll.0, SCROLL);
    assert_eq!(st.session.selection, Selection::Commit(picked));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_filtered_graph_anchors_on_the_matches() {
    let (mut st, dir) = scrolled("anchor-filtered", 60, 100);
    let picked = commit_at(&st, 14);
    st.view.show_search = true;
    st.view.tab.search = "commit".into();
    st.session.search.filter = true;
    crate::update::search::recompute(&mut st);
    assert_eq!(st.session.search.matches.len(), 60, "every commit matches");
    commit_empty(&dir, &["commit new 1", "commit new 2", "other"]);
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    reload_with(&mut st, &dir, 100, LoadKind::Refresh);

    assert_eq!(st.session.search.matches.len(), 62, "\"other\" is filtered out");
    anchored_at(&st, SCROLL + 2.0 * ROW_H);
    assert_eq!(st.session.selection, Selection::Commit(picked));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pending_jump_moves_with_the_rows() {
    let (mut st, dir) = scrolled("anchor-jump", 60, 100);
    let picked = commit_at(&st, 14);
    st.view.tab.graph_scroll = (0.0, 400.0);
    st.view.scroll_to_row = Some(10);
    commit_empty(&dir, &["on top 1", "on top 2", "on top 3"]);
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    reload_with(&mut st, &dir, 100, LoadKind::Refresh);
    anchored_at(&st, 13.0 * ROW_H);
    assert_eq!(st.session.selection, Selection::Commit(picked));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_never_anchors() {
    let (mut st, dir) = scrolled("anchor-open", 60, 100);
    commit_empty(&dir, &["on top 1", "on top 2", "on top 3"]);
    reload_with(&mut st, &dir, 100, LoadKind::Open);
    assert_eq!(st.view.scroll_to_row, Some(0));
    assert_eq!(st.view.scroll_offset, None);
    let _ = std::fs::remove_dir_all(&dir);
}
