//! Loading more of a truncated history when the graph is scrolled near its end: once per load,
//! only when the preference allows, behind a running refresh, and keeping the selection and
//! scroll. A failed load for more is in load_more_retry_tests.rs.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{LoadMsg, Msg, RepoMsg, SelectionMsg};
use crate::state::refresh::Depth;
use crate::state::{AppState, MAX_COMMITS, Selection};
use std::path::PathBuf;
use std::time::Instant;

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

fn loads(fx: &[Effect]) -> Vec<&Effect> {
    fx.iter().filter(|e| matches!(e, Effect::LoadRepo { .. })).collect()
}

fn near_end(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::Load(LoadMsg::NearHistoryEnd), Instant::now())
}

fn load_more(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::Load(LoadMsg::LoadMoreCommits), Instant::now())
}

fn truncate(st: &mut AppState) {
    st.snap.as_mut().unwrap().truncated = true;
}

/// The one full load in `fx`: the commit count it walks.
fn one_full_load(fx: &[Effect]) -> usize {
    let [Effect::LoadRepo { opts, known: None, kind: LoadKind::Refresh, .. }] = &loads(fx)[..] else { panic!("one full load: {fx:?}") };
    opts.max_commits
}

#[test]
fn scrolling_near_the_end_loads_more_once() {
    let (mut st, dir) = opened("more-once");
    let limit = st.settings.commit_limit();
    truncate(&mut st);
    let fx = near_end(&mut st);
    assert_eq!(one_full_load(&fx), 2 * limit);
    assert!(st.loading_more());
    assert!(st.shows_loading_more_row());
    for _ in 0..2 {
        assert!(near_end(&mut st).is_empty(), "a load for more is running");
    }
    assert_eq!(st.refresh.extra_commits, limit);

    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().loaded_with.max_commits, 2 * limit);
    assert!(!st.loading_more());
    assert!(!st.shows_loading_more_row());
    assert!(!st.snap.as_ref().unwrap().truncated, "the fixture is shorter than the limit");
    assert!(near_end(&mut st).is_empty(), "the whole history is loaded");

    truncate(&mut st);
    assert_eq!(one_full_load(&near_end(&mut st)), 3 * limit);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scrolling_loads_nothing_when_off_or_complete() {
    let (mut st, dir) = opened("more-off");
    assert!(near_end(&mut st).is_empty(), "the whole history is shown already");
    assert_eq!(st.refresh.extra_commits, 0);

    truncate(&mut st);
    st.settings.load_more_on_scroll = false;
    assert!(near_end(&mut st).is_empty(), "the preference is off");
    assert_eq!(st.refresh.extra_commits, 0);
    let fx = load_more(&mut st);
    assert_eq!(one_full_load(&fx), 2 * st.settings.commit_limit(), "the Load more link still works");
    finish(&mut st, fx);

    st.settings.load_more_on_scroll = true;
    truncate(&mut st);
    st.snap.as_mut().unwrap().loaded_with.max_commits = MAX_COMMITS;
    assert!(near_end(&mut st).is_empty(), "at the cap");
    assert!(load_more(&mut st).is_empty(), "at the cap");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scrolling_while_a_refresh_runs_queues_one_load() {
    let (mut st, dir) = opened("more-queued");
    let limit = st.settings.commit_limit();
    let running = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert!(st.loading);
    truncate(&mut st);
    assert!(loads(&near_end(&mut st)).is_empty(), "waits for the running load");
    assert_eq!(st.refresh.queued, Some(Depth::Checked));
    assert!(st.loading_more());
    assert!(near_end(&mut st).is_empty());
    assert_eq!(st.refresh.extra_commits, limit);

    // The first load finishes: the queued one runs, once, with the higher limit.
    let load = running.into_iter().find(|e| matches!(e, Effect::LoadRepo { .. })).unwrap();
    let msg = crate::runner::execute(load).unwrap();
    let fx = update(&mut st, msg, Instant::now());
    assert_eq!(one_full_load(&fx), 2 * limit);
    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().loaded_with.max_commits, 2 * limit);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn loading_more_keeps_the_selection_and_scroll() {
    let (mut st, dir) = opened("more-keeps");
    git(&dir, &["commit", "-q", "-am", "second"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let oids: Vec<_> = st.snap.as_ref().unwrap().commits.iter().map(|c| c.oid).collect();
    assert_eq!(oids.len(), 2);

    truncate(&mut st);
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(oids[1])));
    st.view.scroll_to_row = None;
    st.view.scroll_offset = None;
    let fx = near_end(&mut st);
    assert_eq!(loads(&fx).len(), 1);
    finish(&mut st, fx);
    assert_eq!(st.selection, Selection::Commit(oids[1]));
    assert_eq!(st.detail.as_ref().map(|d| d.oid), Some(oids[1]));
    assert_eq!(st.view.scroll_to_row, None);
    assert_eq!(st.view.scroll_offset, None);

    // Two commits selected stay selected.
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(oids[0])));
    assert!(matches!(&st.selection, Selection::Commits(m) if m.len() == 2));
    truncate(&mut st);
    let fx = near_end(&mut st);
    assert_eq!(loads(&fx).len(), 1);
    finish(&mut st, fx);
    let Selection::Commits(m) = &st.selection else { panic!("still two commits: {:?}", st.selection) };
    assert!(m.contains(oids[0]) && m.contains(oids[1]));
    assert_eq!(st.view.scroll_to_row, None);
    let _ = std::fs::remove_dir_all(&dir);
}
