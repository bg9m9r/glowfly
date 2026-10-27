//! Loading more of a real truncated history (`long_repo`): it goes on from the walk
//! (`LoadKind::More`, answered by `LoadMsg::HistoryExtended`) without a full load or a status
//! read, falls back to a full refresh when the refs moved, reads the status after a refresh
//! merged into it or a status read it dropped, holds a failed page, ignores an old one, frees a
//! large history's old commits and lets an index jump land.

use super::history_index_tests::{past_the_delay, run_build};
use super::tests::{drive, finish, finish_keeping, git, long_repo, state};
use super::update;
use crate::effect::{BackgroundEffect, Effect, LoadKind};
use crate::msg::{LoadMsg, Msg, RepoMsg, SearchMsg, WatchMsg};
use crate::state::load_more::MoreRow;
use crate::state::refresh::Depth;
use crate::state::{AppState, Selection};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// `long_repo(name, count)` opened with a commit limit of `limit`.
fn opened(name: &str, count: usize, limit: usize) -> (AppState, PathBuf) {
    let dir = long_repo(name, count);
    let mut st = state();
    st.settings.initial_commits = limit;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.truncated && snap.frontier.is_some() && snap.commits.len() == limit, "cut short with a frontier");
    (st, dir)
}

fn load_more(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::Load(LoadMsg::LoadMoreCommits), Instant::now())
}

fn loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadRepo { .. })).count()
}

fn status_reads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadStatus { .. })).count()
}

/// The one load in `fx`, which goes on from the walk.
#[track_caller]
fn one_more(fx: Vec<Effect>) -> Effect {
    let mut found: Vec<Effect> = fx.into_iter().filter(|e| matches!(e, Effect::LoadRepo { .. })).collect();
    assert_eq!(found.len(), 1, "one load: {found:?}");
    let load = found.pop().unwrap();
    let Effect::LoadRepo { kind: LoadKind::More, known: Some(_), prior: Some(p), .. } = &load else { panic!("a load for more: {load:?}") };
    assert!(p.frontier.is_some(), "with the walk's frontier");
    load
}

/// Runs `load` and checks the runner answered with the next commits of the walk (like a full
/// load's, the answer is kept to its tab by the load generation).
#[track_caller]
fn run_page(load: Effect) -> Msg {
    let msg = crate::runner::execute(load).unwrap();
    assert!(matches!(msg, Msg::Load(LoadMsg::HistoryExtended { result: Ok(_), .. })), "a page, not a full load");
    msg
}

fn page_of(msg: Msg) -> Box<glowfly_core::repo::HistoryPage> {
    let Msg::Load(LoadMsg::HistoryExtended { result: Ok(page), .. }) = msg else { panic!("a page") };
    page
}

#[test]
fn load_more_goes_on_from_the_walk_without_a_full_load() {
    let (mut st, dir) = opened("resume-more", 1200, 500);
    let picked = st.snap.as_ref().unwrap().commits[10].oid;
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(picked)));
    st.view.scroll_to_row = None;

    let fx = load_more(&mut st);
    assert_eq!(status_reads(&fx), 0);
    let load = one_more(fx);
    let Effect::LoadRepo { opts, .. } = &load else { unreachable!() };
    assert_eq!(opts.max_commits, 1000);
    let opts = opts.clone();
    let fx = update(&mut st, run_page(load), Instant::now());
    assert_eq!((loads(&fx), status_reads(&fx)), (0, 0), "nothing else is read: {fx:?}");
    finish(&mut st, fx);

    let snap = st.snap.as_ref().unwrap();
    let fresh = glowfly_core::repo::load(&dir, &opts).unwrap();
    let oids = |s: &glowfly_core::repo::Snapshot| s.commits.iter().map(|c| c.oid).collect::<Vec<_>>();
    assert_eq!(oids(snap), oids(&fresh));
    assert_eq!((snap.commits.len(), snap.loaded_with.max_commits, snap.truncated), (1000, 1000, true));
    assert!(!st.loading && !st.loading_more());
    assert_eq!(st.session.selection, Selection::Commit(picked));
    assert_eq!(st.view.scroll_to_row, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_more_after_refs_moved_loads_in_full() {
    let (mut st, dir) = opened("resume-moved", 1200, 500);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "on top"]);
    let msg = crate::runner::execute(one_more(load_more(&mut st))).unwrap();
    let Msg::Repo(RepoMsg::RepoLoaded { kind: LoadKind::Refresh, result: Ok(snap), .. }) = msg else { panic!("a full refresh") };
    assert_eq!(&*snap.commits[0].summary, "on top");
    assert_eq!(snap.commits.len(), 1000);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_merged_into_load_more_reads_the_status_after() {
    let (mut st, dir) = opened("resume-status", 1200, 500);
    let running = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert!(st.loading);
    let first = running.into_iter().find(|e| matches!(e, Effect::LoadRepo { .. })).unwrap();
    assert_eq!(loads(&load_more(&mut st)), 0, "waits for the running load");
    assert_eq!(st.refresh.queued, Some(Depth::More));
    update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert_eq!(st.refresh.queued, Some(Depth::Checked));

    let fx = update(&mut st, crate::runner::execute(first).unwrap(), Instant::now());
    assert!(st.refresh.status_after);
    let fx = update(&mut st, run_page(one_more(fx)), Instant::now());
    assert_eq!(status_reads(&fx), 1, "the refresh's status read: {fx:?}");
    assert!(!st.refresh.status_after);
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), 1000);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_page_is_held_and_tried_again_as_more() {
    let (mut st, dir) = opened("resume-failed", 1200, 500);
    let t0 = Instant::now();
    let fail = |st: &mut AppState| {
        let failed = LoadMsg::HistoryExtended { generation: st.load_gen, result: Err("walk failed".into()) };
        update(st, Msg::Load(failed), t0)
    };
    one_more(load_more(&mut st));
    assert_eq!(loads(&fail(&mut st)), 0);
    assert_eq!(st.refresh.more_failed.as_deref(), Some("walk failed"));
    assert_eq!(st.more_row(), Some(MoreRow::Failed { error: "walk failed", retrying: true }));
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), 500);

    one_more(update(&mut st, Msg::Load(LoadMsg::RetryLoadMore), t0));
    fail(&mut st);
    let later = t0 + Duration::from_secs(60);
    one_more(update(&mut st, Msg::Tick(later), later));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_old_page_is_ignored() {
    let (mut st, dir) = opened("resume-old", 1200, 500);
    let page = page_of(run_page(one_more(load_more(&mut st))));
    let old_gen = st.load_gen;
    // F5 supersedes the load for more before its page arrives.
    let reload = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert!(st.load_gen > old_gen);
    let before = std::sync::Arc::clone(&st.snap.as_ref().unwrap().commits);
    let stale = LoadMsg::HistoryExtended { generation: old_gen, result: Err("old".into()) };
    assert!(update(&mut st, Msg::Load(stale), Instant::now()).is_empty());
    assert!(st.loading && st.refresh.more_failed.is_none());
    assert!(std::sync::Arc::ptr_eq(&before, &st.snap.as_ref().unwrap().commits));

    // A page made for the history the reload replaced asks for a full load.
    finish(&mut st, reload);
    let shown = std::sync::Arc::clone(&st.snap.as_ref().unwrap().commits);
    let replayed = LoadMsg::HistoryExtended { generation: st.load_gen, result: Ok(page) };
    let fx = update(&mut st, Msg::Load(replayed), Instant::now());
    let full = fx.iter().filter(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, known: None, .. })).count();
    assert_eq!(full, 1, "{fx:?}");
    assert!(std::sync::Arc::ptr_eq(&shown, &st.snap.as_ref().unwrap().commits));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_large_page_frees_the_old_commits_and_asks_for_the_relayout_again() {
    let (mut st, dir) = opened("resume-large", 40_100, 20_000);
    let msg = run_page(one_more(load_more(&mut st)));
    // Request 1 was asked of a worker for the history before the page.
    (st.refresh.relayouts, st.refresh.relayout) = (1, Some(1));
    let fx = update(&mut st, msg, Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Background(BackgroundEffect::Free(_)))), "the old commits are freed on a worker");
    let asked: Vec<u64> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Background(BackgroundEffect::Relayout { request, .. }) => Some(*request),
            _ => None,
        })
        .collect();
    let [request] = asked[..] else { panic!("one relayout: {asked:?}") };
    assert_ne!(request, 1);
    assert_eq!(st.refresh.relayout, Some(request));
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), 40_000);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_index_jump_lands_after_a_resumed_page() {
    let dir = long_repo("resume-jump", 1200);
    let mut st = state();
    st.settings.initial_commits = 500;
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    let mut kept = finish_keeping(&mut st, fx);
    kept.extend(past_the_delay(&mut st));
    let fx = run_build(&mut st, kept);
    finish(&mut st, fx);
    assert!(st.session.search.index.ready.is_some());

    st.view.show_search = true;
    st.session.tab_view.search = "beacon".into();
    drive(&mut st, Msg::Search(SearchMsg::QueryChanged));
    drive(&mut st, Msg::Search(SearchMsg::Next));
    let summary = |st: &AppState| {
        let Selection::Commit(oid) = st.session.selection else { return String::new() };
        let snap = st.snap.as_ref().unwrap();
        snap.commits[snap.row_of[&oid]].summary.to_string()
    };
    assert_eq!(summary(&st), "commit 1199 beacon");

    let load = one_more(update(&mut st, Msg::Search(SearchMsg::Next), Instant::now()));
    assert!(st.session.search.index.jump.is_some());
    let fx = update(&mut st, run_page(load), Instant::now());
    finish(&mut st, fx);
    assert_eq!(summary(&st), "commit 0 needle beacon", "selected once its page is in");
    assert_eq!(st.session.search.index.jump, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_status_read_dropped_by_load_more_is_read_after_the_page() {
    let (mut st, dir) = opened("resume-dropped", 1200, 500);
    std::fs::write(dir.join("new.txt"), "x").unwrap();
    let workdir = st.workdir().unwrap();
    let saved = WatchMsg::RepoChanged { full: false, paths: Some(vec!["new.txt".into()]), last_event: Instant::now() };
    let read = update(&mut st, Msg::Scoped { workdir, msg: Box::new(Msg::Watch(saved)) }, Instant::now());
    assert_eq!(status_reads(&read), 1);

    // The load for more starts before the read lands, which is then out of date.
    let load = one_more(load_more(&mut st));
    assert!(st.refresh.status_after);
    let read = read.into_iter().find(|e| matches!(e, Effect::LoadStatus { .. })).unwrap();
    update(&mut st, crate::runner::execute(read).unwrap(), Instant::now());
    let fx = update(&mut st, run_page(load), Instant::now());
    assert_eq!(status_reads(&fx), 1, "read again: {fx:?}");
    finish(&mut st, fx);
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "new.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_queued_before_a_chained_load_more_reads_the_status() {
    let (mut st, dir) = opened("resume-chained", 3000, 500);
    st.settings.load_more_on_scroll = true;
    st.view.show_search = true;
    st.session.tab_view.search = "needle".into();
    let mut fx = update(&mut st, Msg::Search(SearchMsg::ToggleFilter), Instant::now());
    fx.extend(update(&mut st, Msg::Search(SearchMsg::QueryChanged), Instant::now()));
    // The filtering search loads further back by itself; a refresh asked meanwhile waits.
    let load = one_more(fx);
    update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert_eq!(st.refresh.queued, Some(Depth::Checked));

    // Its page chains the next batch, a load for more that reads the status after its page.
    let fx = update(&mut st, run_page(load), Instant::now());
    assert_eq!(status_reads(&fx), 0);
    let load = one_more(fx);
    assert!(st.refresh.status_after);
    let fx = update(&mut st, run_page(load), Instant::now());
    assert_eq!(status_reads(&fx), 1, "the refresh's status read: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
