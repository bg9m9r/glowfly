//! A failed load for more history: kept for the graph's row, tried again by itself twice after
//! a delay, at once by Try again or Load more with the same commit count, and given up by any
//! load that starts meanwhile, a success or a tab switch.

use super::tests::{drive, finish, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{LoadMsg, Msg, RepoMsg, TabsMsg};
use crate::state::AppState;
use crate::state::load_more::MoreRow;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

fn truncate(st: &mut AppState) {
    st.snap.as_mut().unwrap().truncated = true;
}

/// The one full load in `fx`: the commit count it walks.
fn one_full_load(fx: &[Effect]) -> usize {
    let [Effect::LoadRepo { opts, known: None, kind: LoadKind::Refresh, .. }] = &loads(fx)[..] else { panic!("one full load: {fx:?}") };
    opts.max_commits
}

/// The running load fails with `error` at `at`.
fn fail(st: &mut AppState, error: &str, at: Instant) -> Vec<Effect> {
    let failed = RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Err(error.into()) };
    update(st, Msg::Repo(failed), at)
}

fn tick(st: &mut AppState, at: Instant) -> Vec<Effect> {
    update(st, Msg::Tick(at), at)
}

fn last_log(st: &AppState) -> &str {
    st.log.last().map_or("", |e| e.text.as_str())
}

/// A truncated history with a load for more that failed at `t0`.
fn failed_once(name: &str, t0: Instant) -> (AppState, PathBuf) {
    let (mut st, dir) = opened(name);
    truncate(&mut st);
    assert_eq!(one_full_load(&near_end(&mut st)), 2 * st.settings.commit_limit());
    assert!(loads(&fail(&mut st, "walk failed", t0)).is_empty());
    (st, dir)
}

#[test]
fn a_failed_load_for_more_is_kept_with_its_reason() {
    let t0 = Instant::now();
    let (mut st, dir) = failed_once("more-kept", t0);
    assert_eq!(st.refresh.more_failed.as_deref(), Some("walk failed"));
    assert_eq!(st.refresh.more_retry_at, Some(t0 + Duration::from_secs(3)));
    assert_eq!(st.more_row(), Some(MoreRow::Failed { error: "walk failed", retrying: true }));
    assert!(!st.shows_loading_more_row());
    assert_eq!(last_log(&st), "walk failed");
    assert!(near_end(&mut st).is_empty(), "no loop");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retries_twice_after_a_delay_then_stops() {
    let t0 = Instant::now();
    let (mut st, dir) = failed_once("more-retries", t0);
    let limit = st.settings.commit_limit();
    let toasts = st.toast_seq;
    assert!(loads(&tick(&mut st, t0 + Duration::from_secs(2))).is_empty(), "not due yet");
    let fx = tick(&mut st, t0 + Duration::from_secs(3));
    assert_eq!(one_full_load(&fx), 2 * limit, "the same count, not another limit's worth");
    assert_eq!(st.refresh.more_retries, 1);
    assert_eq!(st.more_row(), Some(MoreRow::Loading));

    let t1 = t0 + Duration::from_secs(4);
    fail(&mut st, "walk failed", t1);
    assert_eq!(st.refresh.more_retry_at, Some(t1 + Duration::from_secs(10)));
    assert_eq!(st.toast_seq, toasts, "a failed automatic retry is logged quietly");
    assert_eq!(last_log(&st), "walk failed");
    assert_eq!(one_full_load(&tick(&mut st, t1 + Duration::from_secs(10))), 2 * limit);
    fail(&mut st, "walk failed", t1 + Duration::from_secs(11));
    assert_eq!(st.refresh.more_retry_at, None, "the retries are used up");
    assert_eq!(st.more_row(), Some(MoreRow::Failed { error: "walk failed", retrying: false }));
    assert!(loads(&tick(&mut st, t0 + Duration::from_secs(600))).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn try_again_and_load_more_retry_the_same_count() {
    let t0 = Instant::now();
    let (mut st, dir) = failed_once("more-try-again", t0);
    let limit = st.settings.commit_limit();
    let fx = update(&mut st, Msg::Load(LoadMsg::RetryLoadMore), t0);
    assert_eq!(one_full_load(&fx), 2 * limit);
    assert_eq!(st.refresh.more_retry_at, None);
    assert!(loads(&tick(&mut st, t0 + Duration::from_secs(5))).is_empty(), "the old timer is gone");

    fail(&mut st, "walk failed", t0);
    let fx = update(&mut st, Msg::Load(LoadMsg::LoadMoreCommits), t0);
    assert_eq!(one_full_load(&fx), 2 * limit, "the header's Load more tries the same load again");

    finish(&mut st, fx);
    assert!(st.refresh.more_failed.is_none());
    truncate(&mut st);
    let fx = update(&mut st, Msg::Load(LoadMsg::LoadMoreCommits), t0);
    assert_eq!(one_full_load(&fx), 3 * limit, "after a success, Load more loads more");
    assert_eq!(st.refresh.more_retries, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_cancels_the_waiting_retry() {
    let t0 = Instant::now();
    let (mut st, dir) = failed_once("more-refresh", t0);
    let limit = st.settings.commit_limit();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), t0);
    assert_eq!(one_full_load(&fx), 2 * limit, "the refresh is the retry");
    assert_eq!((st.refresh.more_failed.as_deref(), st.refresh.more_retry_at), (None, None));
    assert!(loads(&tick(&mut st, t0 + Duration::from_secs(5))).is_empty(), "no second load");

    finish(&mut st, fx);
    assert_eq!((st.refresh.more_failed.as_deref(), st.refresh.more_retry_at, st.refresh.more_retries), (None, None, 0));
    truncate(&mut st);
    assert_eq!(loads(&near_end(&mut st)).len(), 1);
    let t1 = t0 + Duration::from_secs(60);
    fail(&mut st, "walk failed", t1);
    assert_eq!(st.refresh.more_retry_at, Some(t1 + Duration::from_secs(3)), "the first delay again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tab_switch_cancels_the_retry() {
    let t0 = Instant::now();
    let (mut st, a) = failed_once("more-tabs-a", t0);
    let b = scratch_repo("more-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!(st.active_tab, 1);
    assert!(st.refresh.more_failed.is_none());
    assert!(loads(&tick(&mut st, t0 + Duration::from_secs(5))).is_empty());

    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(st.active_tab, 0);
    assert_eq!(st.more_row(), None, "returning shows no failure");
    assert!(loads(&tick(&mut st, t0 + Duration::from_secs(5))).is_empty());
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn an_ordinary_refresh_failure_is_not_kept() {
    let (mut st, dir) = opened("more-ordinary");
    let t0 = Instant::now();
    assert_eq!(loads(&update(&mut st, Msg::Repo(RepoMsg::Refresh), t0)).len(), 1);
    fail(&mut st, "refresh failed", t0);
    assert_eq!((st.refresh.more_failed.as_deref(), st.refresh.more_retry_at), (None, None));
    assert_eq!(last_log(&st), "refresh failed");
    assert_eq!(st.more_row(), None);
    let _ = std::fs::remove_dir_all(&dir);
}
