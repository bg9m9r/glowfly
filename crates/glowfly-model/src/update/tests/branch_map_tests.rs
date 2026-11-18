//! The Branch map without the runner (update/branch_map.rs): opening it asks for one read and
//! closing it stops that read, a plan with nothing to measure runs nothing, results land by their
//! generation, trial merges are remembered by their pair of tips, and a refresh reads again only
//! when a tip moved.

use super::hints_tests::load_kind;
use super::tests::{git, said, scratch_repo, state};
use super::update;
use crate::branch_map::{BranchMapJob, BranchMapMsg};
use crate::effect::partial::StopKey;
use crate::effect::{ControlEffect, Effect, LoadKind};
use crate::journal::JournalMsg;
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::branch_map::tests::{oid, sample};
use crate::state::layout::{ToolView, tool_view};
use glowfly_core::branch_map::{Empty, Forecast, Pair, pair_key, request};
use glowfly_core::branch_ref::BranchRef;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, msg: BranchMapMsg) -> Vec<Effect> {
    update(st, Msg::BranchMap(msg), Instant::now())
}

/// A repository with `main` and `feature` (one commit ahead), loaded and open in a state.
fn with_branches(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "b.txt");
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    st.settings.welcome_done = true;
    load_kind(&mut st, &dir, LoadKind::Open);
    (st, dir)
}

fn commit(dir: &Path, file: &str) {
    std::fs::write(dir.join(file), "x\n").unwrap();
    git(dir, &["add", "--", file]);
    git(dir, &["commit", "-q", "-m", file]);
}

fn jobs(fx: &[Effect]) -> Vec<&BranchMapJob> {
    fx.iter().filter_map(|e| if let Effect::BranchMap(j) = e { Some(j) } else { None }).collect()
}

fn stops(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Control(ControlEffect::StopGit(StopKey::BranchMap(g))) => Some(*g),
            _ => None,
        })
        .collect()
}

fn generation(st: &AppState) -> u64 {
    st.session.branch_map.view.as_ref().unwrap().generation
}

/// The read now asked for finishes with `map`.
fn land(st: &mut AppState, map: glowfly_core::branch_map::BranchMap) {
    let generation = generation(st);
    run(st, BranchMapMsg::Loaded { generation, result: Ok(Box::new(map)) });
}

/// Reloads the snapshot of `dir` as a refresh would, returning the effects.
fn refresh(st: &mut AppState, dir: &Path) -> Vec<Effect> {
    let snap = Box::new(glowfly_core::repo::load(dir, &glowfly_core::repo::LoadOptions::all(100)).unwrap());
    st.loading = true;
    update(st, Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(snap) }), Instant::now())
}

/// Moves `feature` on by a commit, leaving `main` checked out.
fn move_feature(dir: &Path, file: &str) {
    git(dir, &["checkout", "-q", "feature"]);
    commit(dir, file);
    git(dir, &["checkout", "-q", "main"]);
}

#[test]
fn open_asks_for_the_map_and_close_stops_it() {
    let (mut st, dir) = with_branches("bm-update-open");
    update(&mut st, Msg::Journal(JournalMsg::Open), Instant::now());
    st.session.gitflow_panel = true;
    let fx = run(&mut st, BranchMapMsg::Open);
    let g = generation(&st);
    let plan = request(st.snap.as_ref().unwrap());
    assert!(plan.is_ok());
    let [job] = jobs(&fx)[..] else { panic!("one job: {fx:?}") };
    assert_eq!((job.workdir.clone(), job.generation), (st.workdir().unwrap(), g));
    assert_eq!(Ok(job.request.clone()), plan);
    assert!(st.session.branch_map.view.as_ref().unwrap().loading);
    assert!(st.session.journal.view.is_none() && !st.session.gitflow_panel, "the views it covers are closed");
    assert_eq!(tool_view(&st), Some(ToolView::BranchMap));
    assert!(!st.graph_shown());

    let fx = run(&mut st, BranchMapMsg::Close);
    assert!(matches!(&fx[..], [Effect::Control(ControlEffect::StopGit(StopKey::BranchMap(s)))] if *s == g), "{fx:?}");
    assert!(st.session.branch_map.view.is_none());
    assert!(st.session.branch_map.generation > g);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_opens_without_a_repository() {
    let mut st = state();
    assert!(run(&mut st, BranchMapMsg::Open).is_empty());
    assert!(st.session.branch_map.view.is_none());
}

#[test]
fn empty_plans_run_nothing() {
    let dir = scratch_repo("bm-update-empty");
    let mut st = state();
    load_kind(&mut st, &dir, LoadKind::Open);
    let fx = run(&mut st, BranchMapMsg::Open);
    assert!(jobs(&fx).is_empty() && stops(&fx).is_empty(), "{fx:?}");
    let view = st.session.branch_map.view.as_ref().unwrap();
    assert_eq!(view.plan, Err(Empty::OneBranch));
    assert!(!view.loading && view.result.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_result_lands_and_selects_the_current_branch() {
    let (mut st, dir) = with_branches("bm-update-result");
    run(&mut st, BranchMapMsg::Open);
    let g = generation(&st);
    run(&mut st, BranchMapMsg::Loaded { generation: g, result: Ok(Box::new(sample())) });
    let view = st.session.branch_map.view.as_ref().unwrap();
    assert!(!view.loading && view.map().is_some());
    assert_eq!(view.selected, Some(BranchRef::Local("feature".into())), "the checked-out branch");

    run(&mut st, BranchMapMsg::Select(BranchRef::Local("nope".into())));
    assert_eq!(st.session.branch_map.view.as_ref().unwrap().selected_row(), Some(1), "a branch that is not a row is ignored");
    run(&mut st, BranchMapMsg::Select(BranchRef::Local("merged".into())));
    assert_eq!(st.session.branch_map.view.as_ref().unwrap().selected_row(), Some(2));

    let fx = run(&mut st, BranchMapMsg::Loaded { generation: g, result: Err(said("could not read")) });
    assert!(fx.is_empty());
    assert!(matches!(st.session.branch_map.view.as_ref().unwrap().result, Some(Err(_))));
    let fx = run(&mut st, BranchMapMsg::Retry);
    let [job] = jobs(&fx)[..] else { panic!("one job: {fx:?}") };
    assert!(job.generation > g && generation(&st) == job.generation);
    assert!(st.session.branch_map.view.as_ref().unwrap().loading);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_later_result_keeps_the_selection_while_its_branch_is_a_row() {
    let (mut st, dir) = with_branches("bm-update-keep");
    run(&mut st, BranchMapMsg::Open);
    land(&mut st, sample());
    run(&mut st, BranchMapMsg::Select(BranchRef::Local("same".into())));
    run(&mut st, BranchMapMsg::Retry);
    land(&mut st, sample());
    assert_eq!(st.session.branch_map.view.as_ref().unwrap().selected, Some(BranchRef::Local("same".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_results_are_dropped() {
    let (mut st, dir) = with_branches("bm-update-stale");
    run(&mut st, BranchMapMsg::Open);
    let first = generation(&st);
    let fx = run(&mut st, BranchMapMsg::Retry);
    assert_eq!(stops(&fx), [first], "the read it replaces is stopped");
    let second = generation(&st);
    assert!(second > first);
    run(&mut st, BranchMapMsg::Loaded { generation: first, result: Ok(Box::new(sample())) });
    let view = st.session.branch_map.view.as_ref().unwrap();
    assert!(view.loading && view.result.is_none(), "an older request's result changes nothing");

    let elsewhere = glowfly_core::paths::temp_dir().join("glowfly-bm-elsewhere");
    let late = || Msg::BranchMap(BranchMapMsg::Loaded { generation: second, result: Ok(Box::new(sample())) });
    assert!(late().on_inactive().is_none());
    update(&mut st, Msg::Scoped { workdir: elsewhere, msg: Box::new(late()) }, Instant::now());
    let view = st.session.branch_map.view.as_ref().unwrap();
    assert!(view.loading && view.result.is_none(), "another repository's result changes nothing");

    // Another repository opens in the tab: the view closes, what was learned goes, and the old
    // read's result is dropped.
    run(&mut st, BranchMapMsg::Loaded { generation: second, result: Ok(Box::new(sample())) });
    assert!(!st.session.branch_map.known.is_empty());
    let other = scratch_repo("bm-update-stale-other");
    load_kind(&mut st, &other, LoadKind::Open);
    assert!(st.session.branch_map.view.is_none() && st.session.branch_map.known.is_empty());
    let fx = run(&mut st, BranchMapMsg::Loaded { generation: second, result: Ok(Box::new(sample())) });
    assert!(fx.is_empty() && st.session.branch_map.view.is_none());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn forecasts_are_cached_by_tip_pair() {
    let (mut st, dir) = with_branches("bm-update-cache");
    run(&mut st, BranchMapMsg::Open);
    let mut map = sample();
    map.pairs.push(Pair { a: 1, b: 2, shared: vec!["c.txt".into()], forecast: Forecast::Clean });
    map.pairs.push(Pair { a: 1, b: 3, shared: vec!["d.txt".into()], forecast: Forecast::Unknown("x".into()) });
    land(&mut st, map);
    let known = &st.session.branch_map.known;
    assert!(known.contains_key(&pair_key(oid(1), oid(2))) && known.contains_key(&pair_key(oid(2), oid(3))));
    assert!(!known.contains_key(&pair_key(oid(2), oid(4))), "a trial merge that failed is tried again");

    run(&mut st, BranchMapMsg::Close);
    let fx = run(&mut st, BranchMapMsg::Open);
    let [job] = jobs(&fx)[..] else { panic!("one job: {fx:?}") };
    assert_eq!(job.known.len(), 2, "the job carries what is known");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_reloads_only_when_the_tips_changed() {
    let (mut st, dir) = with_branches("bm-update-refresh");
    run(&mut st, BranchMapMsg::Open);
    land(&mut st, sample());
    let fx = refresh(&mut st, &dir);
    assert!(jobs(&fx).is_empty() && stops(&fx).is_empty(), "nothing moved: {fx:?}");

    move_feature(&dir, "c.txt");
    let fx = refresh(&mut st, &dir);
    let [job] = jobs(&fx)[..] else { panic!("one job: {fx:?}") };
    assert!(stops(&fx).is_empty(), "nothing was running");
    let view = st.session.branch_map.view.as_ref().unwrap();
    assert!(view.loading && view.map().is_some(), "the old map stays on screen meanwhile");
    assert_eq!(view.generation, job.generation);

    let running = job.generation;
    move_feature(&dir, "d.txt");
    let fx = refresh(&mut st, &dir);
    assert_eq!(stops(&fx), [running], "the running read is stopped");
    assert_eq!(jobs(&fx).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
