//! A filtering search loading further back while its matches don't fill the graph: doubling
//! batches, one load at a time, a budget per query, only with the filter and the preference on,
//! after the path search, resumed after a failure and started again after a tab switch.

use super::tests::{drive, finish, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{LoadMsg, Msg, RepoMsg, SearchMsg, TabsMsg};
use crate::state::AppState;
use crate::state::load_more::{DEEPER_BATCHES, FILL_ROWS_FALLBACK, Further, Reach};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::commit_query::CommitQuery;

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

/// `opened`, with its history marked as cut at the commit limit.
fn truncated(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = opened(name);
    st.snap.as_mut().unwrap().truncated = true;
    (st, dir)
}

fn loads(fx: &[Effect]) -> Vec<&Effect> {
    fx.iter().filter(|e| matches!(e, Effect::LoadRepo { .. })).collect()
}

/// The one full load in `fx`: the commit count it walks.
fn one_full_load(fx: &[Effect]) -> usize {
    let [Effect::LoadRepo { opts, known: None, kind: LoadKind::Refresh, .. }] = &loads(fx)[..] else { panic!("one full load: {fx:?}") };
    opts.max_commits
}

fn send(st: &mut AppState, msg: SearchMsg) -> Vec<Effect> {
    update(st, Msg::Search(msg), Instant::now())
}

/// The bar shown with `q` typed into it and the filter on.
fn filter(st: &mut AppState, q: &str) -> Vec<Effect> {
    st.view.show_search = true;
    st.view.search = q.into();
    let mut fx = Vec::new();
    if !st.search.filter {
        fx.extend(send(st, SearchMsg::ToggleFilter));
    }
    fx.extend(send(st, SearchMsg::QueryChanged));
    fx
}

/// Runs the load in `fx` and hands back its snapshot as cut short again: the effects that
/// raised.
fn finish_truncated(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { .. })).expect("a load");
    let Some(Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result: Ok(mut snap) })) = crate::runner::execute(load) else {
        panic!("the load succeeds")
    };
    snap.truncated = true;
    update(st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result: Ok(snap) }), Instant::now())
}

fn batches(st: &AppState) -> u8 {
    st.refresh.deeper.as_ref().map_or(0, |d| d.batches)
}

#[test]
fn a_filter_with_too_few_matches_loads_further_back() {
    let (mut st, dir) = truncated("deeper-first");
    let limit = st.settings.commit_limit();
    let fx = filter(&mut st, "zzz-nothing");
    assert_eq!(one_full_load(&fx), 2 * limit);
    assert!(st.searching_older());
    assert_eq!(st.search_reach(), Reach::Searching);
    finish(&mut st, fx);
    assert!(!st.loading, "the fixture is complete, so nothing more loads");
    assert_eq!(st.search_reach(), Reach::Whole { shallow: false });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn batches_double_and_stop_at_the_budget() {
    let (mut st, dir) = truncated("deeper-double");
    let limit = st.settings.commit_limit();
    let mut fx = filter(&mut st, "zzz-nothing");
    let mut asked = Vec::new();
    while !loads(&fx).is_empty() {
        asked.push(one_full_load(&fx) / limit);
        fx = finish_truncated(&mut st, fx);
    }
    assert_eq!(asked, [2, 4, 8, 16, 32]);
    assert_eq!(asked.len(), usize::from(DEEPER_BATCHES));
    assert_eq!(st.search_reach(), Reach::Newest(Further::KeepSearching));

    let fx = update(&mut st, Msg::Load(LoadMsg::SearchFurther), Instant::now());
    assert_eq!(one_full_load(&fx), 64 * limit);
    assert_eq!(batches(&st), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn enough_matches_stop_it() {
    let (mut st, dir) = truncated("deeper-enough");
    st.view.graph_fit_rows = 1;
    assert!(loads(&filter(&mut st, "first")).is_empty(), "the one match fills the graph");
    assert_eq!(st.search.matches.len(), 1);
    st.view.graph_fit_rows = 5;
    assert_eq!(loads(&send(&mut st, SearchMsg::QueryChanged)).len(), 1, "five rows want more");
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = truncated("deeper-fallback");
    assert_eq!(st.view.graph_fit_rows, 0);
    assert_eq!(st.fill_rows(), FILL_ROWS_FALLBACK);
    assert_eq!(loads(&filter(&mut st, "first")).len(), 1, "before the first frame, 20 rows are wanted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_or_cleared_query_stops_the_old_search() {
    let (mut st, dir) = truncated("deeper-new-query");
    let fx = filter(&mut st, "zzz");
    assert_eq!(loads(&fx).len(), 1);
    send(&mut st, SearchMsg::Clear);
    assert_eq!(st.refresh.deeper, None);
    assert!(loads(&finish_truncated(&mut st, fx)).is_empty(), "the running load is kept, but goes no further");

    let fx = filter(&mut st, "aaa");
    assert_eq!(loads(&fx).len(), 1);
    assert!(loads(&filter(&mut st, "bbb")).is_empty(), "one load at a time");
    let next = finish_truncated(&mut st, fx);
    assert_eq!(loads(&next).len(), 1);
    let deeper = st.refresh.deeper.as_ref().unwrap();
    assert_eq!((&deeper.query, deeper.batches), (&CommitQuery::parse("bbb"), 1), "a fresh budget for the new query");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_while_filtering_with_the_preference_on() {
    let (mut st, dir) = truncated("deeper-only");
    st.view.show_search = true;
    st.view.search = "zzz".into();
    assert!(loads(&send(&mut st, SearchMsg::QueryChanged)).is_empty(), "the query only dims rows");
    assert_eq!(st.refresh.deeper, None);

    st.settings.load_more_on_scroll = false;
    assert!(loads(&filter(&mut st, "zzz")).is_empty(), "the preference is off");
    assert_eq!(st.search_reach(), Reach::Newest(Further::LoadMore));

    st.settings.load_more_on_scroll = true;
    let fx = send(&mut st, SearchMsg::QueryChanged);
    assert_eq!(loads(&fx).len(), 1);
    assert!(loads(&send(&mut st, SearchMsg::Close)).is_empty());
    assert_eq!(st.refresh.deeper, None, "the bar is hidden");
    assert!(loads(&finish_truncated(&mut st, fx)).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn path_terms_wait_for_the_file_search() {
    let (mut st, dir) = truncated("deeper-paths");
    let mut fx = filter(&mut st, "path:nowhere/");
    assert!(fx.iter().any(|e| matches!(e, Effect::SearchPaths { .. })));
    assert!(loads(&fx).is_empty(), "the matches aren't final yet");
    let mut loaded = 0;
    while let Some(search) = fx.iter().position(|e| matches!(e, Effect::SearchPaths { .. })) {
        assert!(st.search.searching_files());
        let reply = crate::runner::execute(fx.swap_remove(search)).expect("a report");
        fx = update(&mut st, reply, Instant::now());
        loaded += loads(&fx).len();
    }
    assert!(!st.search.searching_files());
    assert_eq!(loaded, 1, "one load once the files are searched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_batch_holds_the_search_and_resumes() {
    let (mut st, dir) = truncated("deeper-failed");
    let limit = st.settings.commit_limit();
    assert_eq!(loads(&filter(&mut st, "zzz")).len(), 1);
    let failed = RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Err("walk failed".into()) };
    update(&mut st, Msg::Repo(failed), Instant::now());
    assert_eq!(st.search_reach(), Reach::Failed("walk failed"));
    assert!(loads(&send(&mut st, SearchMsg::QueryChanged)).is_empty(), "the failure holds the search");

    let fx = update(&mut st, Msg::Load(LoadMsg::RetryLoadMore), Instant::now());
    assert_eq!(one_full_load(&fx), 2 * limit);
    let next = finish_truncated(&mut st, fx);
    assert_eq!(one_full_load(&next), 4 * limit, "the search goes on with its budget");
    assert_eq!(batches(&st), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tab_switch_drops_the_deeper_search() {
    let (mut st, a) = truncated("deeper-tabs-a");
    let fx = filter(&mut st, "zzz");
    // The first batch lands, so the tab holds more than the limit when it is left.
    assert_eq!(loads(&finish_truncated(&mut st, fx)).len(), 1);
    assert_eq!(batches(&st), 2);
    let b = scratch_repo("deeper-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!(st.active_tab, 1);

    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(st.refresh.deeper, None, "dropped by the switch");
    assert!(st.filtering(), "the tab keeps its search");
    let next = finish_truncated(&mut st, fx);
    assert_eq!(one_full_load(&next), 2 * st.settings.commit_limit());
    assert_eq!(batches(&st), 1, "started again with a fresh budget");
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}
