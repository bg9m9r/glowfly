//! Building a tab's whole-history index: when a build starts (and when not), fresh builds and
//! silent updates from the previous index, stale and failed builds, parked tabs, closing tabs,
//! another repository in the tab and the preference.

use super::tests::{finish, finish_keeping, git, long_repo, scratch_repo, state};
use super::update;
use crate::effect::{Effect, IndexEffect};
use crate::msg::{Msg, RepoMsg, SearchMsg, TabsMsg};
use crate::state::history_index::{FIRST_BUILD_DELAY, HistoryCount};
use crate::state::tabs::Parked;
use crate::state::{AppState, Tab};
use glowfly_core::commit_index::{CommitIndex, IndexKey};
use glowfly_core::commit_query::CommitQuery;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// `long_repo` opened with the smallest commit limit: its graph is cut short, and its first
/// index build waits for its delay. Returns the effects `finish` skipped.
pub(super) fn opened_long_waiting(name: &str) -> (AppState, PathBuf, Vec<Effect>) {
    let dir = long_repo(name, 620);
    let mut st = state();
    st.settings.initial_commits = 500;
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    assert!(st.snap.as_ref().is_some_and(|s| s.truncated && s.commits.len() == 500));
    (st, dir, kept)
}

/// Ticks `st` through the first build's delay; the effects `finish` skipped.
pub(super) fn past_the_delay(st: &mut AppState) -> Vec<Effect> {
    let t = Instant::now();
    let mut kept = Vec::new();
    for tick in [t, t + FIRST_BUILD_DELAY] {
        let fx = update(st, Msg::Tick(tick), Instant::now());
        kept.extend(finish_keeping(st, fx));
    }
    kept
}

/// `opened_long_waiting` ticked past the delay: the effects `finish` skipped (the index build
/// among them).
pub(super) fn opened_long(name: &str) -> (AppState, PathBuf, Vec<Effect>) {
    let (mut st, dir, mut kept) = opened_long_waiting(name);
    kept.extend(past_the_delay(&mut st));
    (st, dir, kept)
}

/// The index builds among `fx`.
pub(super) fn builds(fx: &[Effect]) -> Vec<&IndexEffect> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Index(b @ IndexEffect::Build { .. }) => Some(b),
            _ => None,
        })
        .collect()
}

fn forgets(fx: &[Effect]) -> Vec<PathBuf> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Index(IndexEffect::Forget { workdir }) => Some(workdir.clone()),
            _ => None,
        })
        .collect()
}

/// Runs the one build in `fx` and hands its result to update; the effects that raised.
pub(super) fn run_build(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let build = fx.into_iter().find(|e| matches!(e, Effect::Index(IndexEffect::Build { .. }))).expect("a build");
    let msg = crate::runner::execute(build).expect("a result");
    update(st, msg, Instant::now())
}

fn send(st: &mut AppState, msg: SearchMsg) -> Vec<Effect> {
    update(st, Msg::Search(msg), Instant::now())
}

fn query(st: &mut AppState, q: &str) -> Vec<Effect> {
    st.view.show_search = true;
    st.view.search = q.into();
    send(st, SearchMsg::QueryChanged)
}

fn log_count(st: &AppState, text: &str) -> usize {
    st.log.iter().filter(|l| l.text.contains(text)).count()
}

#[test]
fn a_truncated_load_starts_a_fresh_build() {
    let (mut st, dir, kept) = opened_long("index-fresh");
    let [IndexEffect::Build { key, base: None, build, workdir }] = builds(&kept)[..] else { panic!("one fresh build: {kept:?}") };
    assert_eq!(key, &IndexKey::of(st.snap.as_ref().unwrap()));
    assert_eq!(Some(workdir), st.workdir().as_ref());
    assert_eq!(st.search.index.building.as_ref().map(|b| (b.build, b.fresh)), Some((*build, true)));
    assert_eq!(st.indexing(), Some((0, None)));
    assert_eq!(st.history_count(), HistoryCount::Loaded, "no query yet");
    query(&mut st, "needle");
    assert_eq!(st.history_count(), HistoryCount::Indexing);
    assert_eq!(st.search.count_text(None, st.history_count()).as_deref(), Some("Indexing history…"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_complete_history_builds_nothing() {
    let dir = scratch_repo("index-complete");
    let mut st = state();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    assert!(builds(&finish_keeping(&mut st, fx)).is_empty());
    assert!(st.search.index.building.is_none() && st.indexing().is_none());
    let _ = std::fs::remove_dir_all(&dir);

    let dir = long_repo("index-off", 620);
    let mut st = state();
    st.settings.initial_commits = 500;
    st.settings.index_history = false;
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    assert!(builds(&finish_keeping(&mut st, fx)).is_empty(), "the preference is off");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn progress_for_another_build_is_ignored() {
    let (mut st, dir, _) = opened_long("index-progress");
    let build = st.search.index.building.as_ref().unwrap().build;
    send(&mut st, SearchMsg::IndexProgress { build: build + 7, indexed: 99, estimate: Some(5) });
    assert_eq!(st.indexing(), Some((0, None)));
    send(&mut st, SearchMsg::IndexProgress { build, indexed: 120, estimate: Some(1300) });
    assert_eq!(st.indexing(), Some((120, Some(1300))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_built_is_ignored() {
    let (mut st, dir, _) = opened_long("index-stale");
    let build = st.search.index.building.as_ref().unwrap().build;
    let index = Arc::new(CommitIndex::from_commits(IndexKey::of(st.snap.as_ref().unwrap()), &[]));
    send(&mut st, SearchMsg::IndexBuilt { build: build - 1, result: Ok(index) });
    assert!(st.search.index.ready.is_none() && st.search.index.building.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn built_index_is_asked_for_the_query() {
    let (mut st, dir, kept) = opened_long("index-asked");
    let fx = run_build(&mut st, kept);
    assert!(fx.iter().all(|e| !matches!(e, Effect::Index(_))), "no query yet: {fx:?}");
    let ready = st.search.index.ready.clone().expect("the index is kept");
    assert_eq!((ready.len(), ready.complete()), (620, true));
    assert!(st.search.index.building.is_none() && st.indexing().is_none());

    let fx = query(&mut st, "needle");
    assert!(fx.iter().any(|e| matches!(e, Effect::Index(IndexEffect::Query { .. }))), "{fx:?}");
    assert_eq!(st.history_count(), HistoryCount::Searching);
    assert_eq!(st.search.count_text(None, st.history_count()).as_deref(), Some("Searching history…"));
    finish(&mut st, fx);
    assert_eq!(st.history_count(), HistoryCount::Whole { positions: &[617, 618, 619], commits: 620, complete: true });
    assert_eq!(st.search.count_text(None, st.history_count()).as_deref(), Some("3 matches"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_refs_update_the_index_with_the_old_one_as_base() {
    let (mut st, dir, kept) = opened_long("index-update");
    run_build(&mut st, kept);
    let ready = st.search.index.ready.clone().unwrap();
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "newer"]);
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    let [IndexEffect::Build { base: Some(base), key, .. }] = builds(&kept)[..] else { panic!("one update: {kept:?}") };
    assert!(Arc::ptr_eq(base, &ready), "built from the previous index");
    assert!(key.covers(st.snap.as_ref().unwrap()));
    assert!(!st.search.index.building.as_ref().unwrap().fresh);
    assert_eq!(st.indexing(), None, "an update shows nothing in the status bar");
    run_build(&mut st, kept);
    assert_eq!(st.search.index.ready.as_ref().map(|r| r.len()), Some(621));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_build_is_logged_once_and_not_retried_for_the_same_history() {
    let (mut st, dir, _) = opened_long("index-failed");
    let build = st.search.index.building.as_ref().unwrap().build;
    send(&mut st, SearchMsg::IndexBuilt { build, result: Err("boom".into()) });
    assert_eq!(log_count(&st, "Could not index the history for search: boom"), 1);
    assert!(st.search.index.building.is_none());
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert!(builds(&finish_keeping(&mut st, fx)).is_empty(), "the same history isn't tried again");

    git(&dir, &["commit", "-q", "--allow-empty", "-m", "newer"]);
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert_eq!(builds(&finish_keeping(&mut st, fx)).len(), 1, "a changed history is");
    assert_eq!(log_count(&st, "Could not index the history"), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A parked tab for `path` holding an index for it.
fn parked_with_index(path: &std::path::Path) -> Tab {
    let mut tab = Tab::unloaded(path.to_path_buf());
    let mut parked = Parked::default();
    parked.commit_search.index.workdir = Some(path.to_path_buf());
    parked.commit_search.index.ready = Some(Arc::new(CommitIndex::from_commits(IndexKey { starts: vec![], shallow: vec![] }, &[])));
    tab.parked = Some(Box::new(parked));
    tab
}

#[test]
fn closing_the_tab_forgets_its_build() {
    let (mut st, dir, _) = opened_long("index-close");
    let other = glowfly_core::paths::temp_dir().join("glowfly-index-close-other");
    st.tabs.push(parked_with_index(&other));
    let fx = update(&mut st, Msg::Tabs(TabsMsg::CloseTab(1)), Instant::now());
    assert_eq!(forgets(&fx), std::slice::from_ref(&other), "a parked tab");
    let workdir = st.workdir().unwrap();
    let fx = update(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)), Instant::now());
    assert_eq!(forgets(&fx), [workdir], "the active tab");

    let (mut st, dir2, _) = opened_long("index-close-others");
    st.tabs.push(parked_with_index(&other));
    let fx = update(&mut st, Msg::Tabs(TabsMsg::CloseOtherTabs(0)), Instant::now());
    assert_eq!(forgets(&fx), std::slice::from_ref(&other), "close others");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn opening_another_repo_in_the_tab_forgets_the_old_one() {
    let (mut st, dir, _) = opened_long("index-replace");
    let old = st.workdir().unwrap();
    let next = scratch_repo("index-replace-next");
    let mut fx = Vec::new();
    super::repo::load(&mut st, next.clone(), crate::effect::LoadKind::Open, &mut fx);
    let mut kept = Vec::new();
    let msg = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { .. })).and_then(crate::runner::execute).unwrap();
    kept.extend(update(&mut st, msg, Instant::now()));
    assert_eq!(forgets(&kept), [old]);
    assert!(st.search.index.ready.is_none() && st.search.index.building.is_none());
    assert_eq!(st.search.index.workdir, st.workdir());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&next);
}

#[test]
fn an_index_built_for_a_parked_tab_is_kept() {
    let (mut st, dir, kept) = opened_long("index-parked");
    let build = kept.into_iter().find(|e| matches!(e, Effect::Index(IndexEffect::Build { .. }))).unwrap();
    // The bar is open with a query when the tab is parked.
    st.view.show_search = true;
    st.view.search = "needle".into();
    st.search.query = CommitQuery::parse("needle");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    finish(&mut st, fx);
    assert_eq!(st.active_tab, 1);
    let result = crate::runner::execute(build).unwrap();
    assert!(matches!(&result, Msg::Scoped { workdir, .. } if *workdir == dir || super::tests::same_place(workdir, &dir)));
    update(&mut st, result, Instant::now());
    let parked = st.tabs[0].parked.as_ref().unwrap();
    assert!(parked.commit_search.index.ready.is_some(), "the parked tab keeps it");
    assert!(parked.commit_search.index.building.is_none());

    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Index(IndexEffect::Query { .. }))), "asked for the query: {fx:?}");
    assert!(builds(&fx).is_empty(), "nothing to build");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turning_the_preference_off_drops_the_index() {
    let (mut st, dir, kept) = opened_long("index-pref");
    run_build(&mut st, kept);
    st.tabs.push(parked_with_index(&glowfly_core::paths::temp_dir().join("glowfly-index-pref-other")));
    st.settings.index_history = false;
    let mut fx = Vec::new();
    super::prefs::sync_live_settings(&mut st, &mut fx);
    assert_eq!(forgets(&fx).len(), 2, "the active and the parked tab: {fx:?}");
    assert!(st.search.index.ready.is_none());
    assert!(st.tabs[1].parked.as_ref().unwrap().commit_search.index.ready.is_none());
    let mut fx = Vec::new();
    super::prefs::sync_live_settings(&mut st, &mut fx);
    assert!(forgets(&fx).is_empty(), "once");

    st.settings.index_history = true;
    let mut fx = Vec::new();
    super::prefs::sync_live_settings(&mut st, &mut fx);
    assert!(builds(&fx).is_empty(), "nothing in memory: the build waits");
    assert_eq!(builds(&past_the_delay(&mut st)).len(), 1, "turning it on starts a build");
    let _ = std::fs::remove_dir_all(&dir);
}
