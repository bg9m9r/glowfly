//! Line counts: a selected commit's asked for once and cached per tab, the working tree's
//! following every status read with at most one request running.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LineCountsEffect};
use crate::msg::{Msg, RepoMsg, StageMsg, TabsMsg};
use crate::state::AppState;
use git2::Oid;
use glowfly_core::repo::{LineCount, WipLineCounts};
use std::path::Path;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn head(st: &AppState) -> Oid {
    st.snap.as_ref().unwrap().commits[0].oid
}

fn commit_asks(fx: &[Effect]) -> Vec<Oid> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LineCounts(LineCountsEffect::Commit { oid, .. }) => Some(*oid),
            _ => None,
        })
        .collect()
}

fn wip_asks(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LineCounts(LineCountsEffect::Wip { generation, .. }) => Some(*generation),
            _ => None,
        })
        .collect()
}

fn n(added: usize, removed: usize) -> LineCount {
    LineCount { added, removed }
}

/// Runs the line count effects in `fx` (and what they lead to) to completion.
fn finish_counts(st: &mut AppState, fx: Vec<Effect>) {
    finish(st, fx.into_iter().filter(|e| matches!(e, Effect::LineCounts(_))).collect());
}

#[test]
fn selecting_a_commit_asks_for_its_counts_once() {
    let dir = scratch_repo("lc-select");
    let mut st = opened(&dir);
    let oid = head(&st);
    let fx = run(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    assert_eq!(commit_asks(&fx), [oid]);
    assert!(commit_asks(&run(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)))).is_empty(), "already on its way");
    finish_counts(&mut st, fx);
    assert_eq!(st.line_counts.commit(oid).and_then(|c| c.get("a.txt")), Some(n(1, 0)));
    assert!(commit_asks(&run(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)))).is_empty(), "cached");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_counts_survive_refresh_and_tab_switch_but_not_open() {
    let dir = scratch_repo("lc-tabs-a");
    let mut st = opened(&dir);
    let oid = head(&st);
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    assert!(st.line_counts.commit(oid).is_some());
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.line_counts.commit(oid).is_some(), "a refresh keeps them");

    // Pending when parked: asked for again on return.
    st.line_counts = Default::default();
    let fx = run(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    assert_eq!(commit_asks(&fx), [oid]);
    let other = scratch_repo("lc-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    assert!(st.line_counts.commit(oid).is_none(), "the other tab has its own");
    let fx = run(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(commit_asks(&fx), [oid], "the request in flight was dropped with the tab");
    finish(&mut st, fx);
    assert!(st.line_counts.commit(oid).is_some());
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(st.line_counts.commit(oid).is_some(), "results survive the round trip");

    // Opening a repository in this tab starts from nothing.
    let mut fx = Vec::new();
    super::repo::load(&mut st, dir.clone(), crate::effect::LoadKind::Open, &mut fx);
    finish(&mut st, fx);
    assert!(st.line_counts.commit(oid).is_none(), "an Open clears them");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn wip_counts_follow_the_status() {
    let dir = scratch_repo("lc-wip");
    let mut st = opened(&dir);
    let w = st.line_counts.wip().expect("counted after the open");
    assert_eq!(w.unstaged.get("a.txt"), Some(n(1, 0)));
    assert_eq!(w.staged.get("a.txt"), None);

    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    let w = st.line_counts.wip().unwrap();
    assert_eq!(w.staged.get("a.txt"), Some(n(1, 0)));
    assert_eq!(w.unstaged.get("a.txt"), None);

    git(&dir, &["commit", "-q", "-m", "second"]);
    let fx = run(&mut st, Msg::Repo(RepoMsg::Refresh));
    finish(&mut st, fx);
    assert!(st.snap.as_ref().is_some_and(|s| !s.is_dirty()));
    let mut fx = Vec::new();
    super::line_counts::status_changed(&mut st, &mut fx);
    assert!(wip_asks(&fx).is_empty(), "a clean tree sends no request");
    assert_eq!(st.line_counts.wip(), Some(&WipLineCounts::default()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_status_read_while_wip_counts_run_is_queued() {
    let dir = scratch_repo("lc-queue");
    let mut st = opened(&dir);
    let mut fx = Vec::new();
    super::line_counts::status_changed(&mut st, &mut fx);
    super::line_counts::status_changed(&mut st, &mut fx);
    let [generation] = wip_asks(&fx)[..] else { panic!("one request: {:?}", wip_asks(&fx)) };

    let stale = run(&mut st, Msg::Repo(RepoMsg::WipLineCountsLoaded { generation: generation - 1, result: Ok(Box::default()) }));
    assert!(wip_asks(&stale).is_empty(), "a stale reply is ignored");
    let fx = run(&mut st, Msg::Repo(RepoMsg::WipLineCountsLoaded { generation, result: Ok(Box::default()) }));
    let [next] = wip_asks(&fx)[..] else { panic!("the queued request is sent") };
    assert!(next > generation);

    let fx = run(&mut st, Msg::Repo(RepoMsg::WipLineCountsLoaded { generation: next, result: Err("boom".into()) }));
    assert!(wip_asks(&fx).is_empty(), "nothing was queued");
    assert!(st.line_counts.wip().is_none(), "a failure shows no counts");
    let mut fx = Vec::new();
    super::line_counts::status_changed(&mut st, &mut fx);
    assert_eq!(wip_asks(&fx).len(), 1, "a failure isn't left busy");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn line_counts_results_for_another_tab_are_dropped() {
    let dir = scratch_repo("lc-scoped");
    let mut st = opened(&dir);
    let oid = head(&st);
    st.line_counts.start_commit(oid);
    let msg = Msg::Repo(RepoMsg::CommitLineCountsLoaded { oid, result: Ok(Default::default()) });
    let elsewhere = glowfly_core::paths::temp_dir().join("glowfly-lc-elsewhere");
    run(&mut st, Msg::Scoped { workdir: elsewhere, msg: Box::new(msg) });
    assert!(st.line_counts.commit(oid).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
