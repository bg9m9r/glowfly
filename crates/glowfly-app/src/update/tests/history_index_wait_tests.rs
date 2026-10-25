//! The whole-history index's timing: a first build waits `FIRST_BUILD_DELAY` after the history
//! loads (and nothing starts when indexing is turned off meanwhile), updates don't wait, and a
//! tab parked for `PARKED_FREE_AFTER` frees a large index and asks for it again when shown.

use super::history_index_tests::{builds, opened_long, opened_long_waiting, run_build};
use super::tests::{finish, git};
use super::update;
use crate::effect::{Effect, IndexEffect};
use crate::msg::{Msg, RepoMsg, SearchMsg, TabsMsg};
use crate::state::history_index::{BuildDelay, FIRST_BUILD_DELAY, FREE_MIN_COMMITS, HistoryCount, PARKED_FREE_AFTER};
use crate::state::tabs::Parked;
use crate::state::{AppState, Tab};
use glowfly_core::commit_index::{CommitIndex, IndexKey};
use glowfly_core::repo::{Author, CommitInfo};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn tick(st: &mut AppState, t: Instant) -> Vec<Effect> {
    update(st, Msg::Tick(t), Instant::now())
}

/// An index of `n` synthetic commits for `key`.
fn index_of(key: IndexKey, n: usize) -> Arc<CommitIndex> {
    let author = Author::new("A", "a@x.org");
    let commits: Vec<CommitInfo> = (0..n as u32)
        .map(|i| {
            let mut id = [0u8; 20];
            id[..4].copy_from_slice(&(i + 1).to_le_bytes());
            CommitInfo {
                oid: git2::Oid::from_bytes(&id).unwrap(),
                summary: Default::default(),
                body: Default::default(),
                author: author.clone(),
                time: 0,
                parents: Default::default(),
            }
        })
        .collect();
    Arc::new(CommitIndex::from_commits(key, &commits))
}

fn no_key() -> IndexKey {
    IndexKey { starts: Vec::new(), shallow: Vec::new() }
}

/// A parked tab for `path` whose index holds `ix`.
fn parked_holding(path: &Path, ix: Option<Arc<CommitIndex>>) -> Tab {
    let mut tab = Tab::unloaded(path.to_path_buf());
    let mut parked = Parked::default();
    parked.commit_search.index.workdir = Some(path.to_path_buf());
    parked.commit_search.index.ready = ix;
    tab.parked = Some(Box::new(parked));
    tab
}

fn parked_ready(st: &AppState, i: usize) -> bool {
    st.tabs[i].parked.as_ref().unwrap().commit_search.index.ready.is_some()
}

#[test]
fn the_first_build_waits_after_open() {
    let (mut st, dir, kept) = opened_long_waiting("index-wait");
    assert!(builds(&kept).is_empty(), "{kept:?}");
    assert_eq!(st.search.index.delay, BuildDelay::Wanted);
    st.view.show_search = true;
    st.view.search = "needle".into();
    update(&mut st, Msg::Search(SearchMsg::QueryChanged), Instant::now());
    assert_eq!(st.history_count(), HistoryCount::Indexing);
    assert_eq!(st.indexing(), None, "the status bar waits for the build");

    let t0 = Instant::now();
    assert!(builds(&tick(&mut st, t0)).is_empty());
    assert_eq!(st.search.index.delay, BuildDelay::Due(t0 + FIRST_BUILD_DELAY));
    assert_eq!(st.index_wake(t0), Some(t0 + FIRST_BUILD_DELAY));
    assert!(builds(&tick(&mut st, t0 + FIRST_BUILD_DELAY - Duration::from_millis(1))).is_empty());
    let fx = tick(&mut st, t0 + FIRST_BUILD_DELAY);
    let [IndexEffect::Build { base: None, .. }] = builds(&fx)[..] else { panic!("one fresh build: {fx:?}") };
    assert!(st.search.index.building.as_ref().is_some_and(|b| b.fresh));
    assert_eq!(st.index_wake(t0), None);
    run_build(&mut st, fx);
    assert!(st.search.index.ready.is_some());

    // With an index in memory, an update starts at once.
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "newer"]);
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let kept = super::tests::finish_keeping(&mut st, fx);
    let [IndexEffect::Build { base: Some(_), .. }] = builds(&kept)[..] else { panic!("one update: {kept:?}") };
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turning_indexing_off_while_waiting_starts_nothing() {
    let (mut st, dir, _) = opened_long_waiting("index-wait-off");
    assert_eq!(st.search.index.delay, BuildDelay::Wanted);
    st.settings.index_history = false;
    let mut fx = Vec::new();
    super::prefs::sync_live_settings(&mut st, &mut fx);
    assert_eq!(st.search.index.delay, BuildDelay::Idle);
    let t0 = Instant::now();
    for t in [t0, t0 + FIRST_BUILD_DELAY, t0 + 2 * FIRST_BUILD_DELAY] {
        assert!(builds(&tick(&mut st, t)).is_empty());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tab_parked_long_frees_its_index_and_reads_it_back_when_shown() {
    let (mut st, dir, _) = opened_long("index-wait-free");
    let key = IndexKey::of(st.snap.as_ref().unwrap());
    st.search.index.building = None;
    st.search.index.ready = Some(index_of(key.clone(), FREE_MIN_COMMITS));
    let s = Instant::now();
    tick(&mut st, s);
    let fx = update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    finish(&mut st, fx);
    assert_eq!(st.active_tab, 1);
    assert_eq!(st.index_wake(s), Some(s + PARKED_FREE_AFTER));

    tick(&mut st, s + PARKED_FREE_AFTER - Duration::from_secs(1));
    assert!(parked_ready(&st, 0));
    let fx = tick(&mut st, s + PARKED_FREE_AFTER);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Index(IndexEffect::Forget { .. }))), "{fx:?}");
    let ix = &st.tabs[0].parked.as_ref().unwrap().commit_search.index;
    assert!(ix.ready.is_none() && ix.answer.is_none());
    assert!(ix.workdir.is_some(), "the repository stays");

    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let mut kept = super::tests::finish_keeping(&mut st, fx);
    assert!(builds(&kept).is_empty(), "the build waits: {kept:?}");
    let t = s + 2 * PARKED_FREE_AFTER;
    kept.extend(tick(&mut st, t));
    kept.extend(tick(&mut st, t + FIRST_BUILD_DELAY));
    let [IndexEffect::Build { base: None, build, .. }] = builds(&kept)[..] else { panic!("one build: {kept:?}") };
    let build = *build;
    assert!(st.search.index.building.as_ref().is_some_and(|b| b.fresh));
    let back = index_of(key, 3);
    update(&mut st, Msg::Search(SearchMsg::IndexBuilt { build, result: Ok(back) }), Instant::now());
    assert!(st.search.index.ready.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_small_or_recently_parked_index_is_kept() {
    let (mut st, dir, _) = opened_long("index-wait-keep");
    let tmp = glowfly_core::paths::temp_dir();
    st.tabs.push(parked_holding(&tmp.join("glowfly-index-keep-small"), Some(index_of(no_key(), 10))));
    st.tabs.push(parked_holding(&tmp.join("glowfly-index-keep-large"), Some(index_of(no_key(), FREE_MIN_COMMITS))));
    let s = Instant::now();
    tick(&mut st, s);
    tick(&mut st, s + Duration::from_secs(4 * 60));
    assert!(parked_ready(&st, 1) && parked_ready(&st, 2), "parked four minutes");
    st.tabs.remove(2);
    tick(&mut st, s + Duration::from_secs(60 * 60));
    assert!(parked_ready(&st, 1), "a small index stays");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_build_landing_for_a_long_parked_tab_is_freed_on_the_next_tick() {
    let (mut st, dir, _) = opened_long("index-wait-landing");
    let other = glowfly_core::paths::temp_dir().join("glowfly-index-landing-other");
    let mut tab = parked_holding(&other, None);
    let ix = &mut tab.parked.as_mut().unwrap().commit_search.index;
    ix.builds = 1;
    ix.building = Some(crate::state::history_index::Building { build: 1, key: no_key(), indexed: 0, estimate: None, fresh: true });
    st.tabs.push(tab);
    let s = Instant::now();
    tick(&mut st, s);
    let landed = SearchMsg::IndexBuilt { build: 1, result: Ok(index_of(no_key(), FREE_MIN_COMMITS)) };
    update(&mut st, Msg::Scoped { workdir: other.clone(), msg: Box::new(Msg::Search(landed)) }, Instant::now());
    assert!(parked_ready(&st, 1), "kept by parked_built");
    tick(&mut st, s + PARKED_FREE_AFTER);
    assert!(!parked_ready(&st, 1), "freed on the next tick");
    let _ = std::fs::remove_dir_all(&dir);
}
