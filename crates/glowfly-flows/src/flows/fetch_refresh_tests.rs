//! End-to-end tests for the refresh after a background fetch (update/refs_check.rs), run through
//! the runner on a repository with a linked worktree: with the watcher live, an auto-fetch that
//! moved nothing reads no status and counts no worktrees, one that moved a ref loads everything,
//! and without a live watcher the status is read and the worktrees counted as before.

use super::refresh_tests::{quiet_job, run_load};
use super::tests::{drive, git};
use super::update;
use super::worktree_counts_tests::opened;
use glowfly_model::effect::{Effect, LoadEffect, LoadKind, WorktreeEffect};
use glowfly_model::msg::{JobMsg, LoadMsg, Msg, RepoMsg, WatchMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::refresh::recount::RECOUNT_EVERY;
use glowfly_model::state::watch::Probe;
use std::path::Path;
use std::time::Instant;

/// The auto-fetch's end in the active tab, when a recount is due: the effects.
fn auto_fetch_ends(st: &mut AppState) -> (Vec<Effect>, Instant) {
    let later = Instant::now() + RECOUNT_EVERY * 2;
    st.job = Some(quiet_job(&st.workdir().unwrap()));
    (update(st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }), later), later)
}

fn live(st: &mut AppState) {
    let workdir = st.workdir().unwrap();
    update(st, Msg::Watch(WatchMsg::Probed { workdir, probe: Probe::Live }), Instant::now());
    assert!(st.watch.live_for(&st.workdir().unwrap()), "the watcher is on the repository and live");
}

fn counts(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Worktree(WorktreeEffect::CountChanges(_))))
}

fn reads(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Status { .. } | LoadEffect::Repo { .. })))
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn an_auto_fetch_that_moved_nothing_reads_no_status_and_counts_no_worktrees() {
    let (mut st, dir, linked) = opened("fetch-refresh-nothing");
    live(&mut st);
    assert_eq!(st.refresh.counting, None, "the first count is finished");
    let (fx, later) = auto_fetch_ends(&mut st);
    let msg = run_load(fx);
    assert!(matches!(msg, Msg::Load(LoadMsg::RefsUnchanged { .. })), "the refs are as they were");
    let fx = update(&mut st, msg, later);
    assert!(!reads(&fx) && !counts(&fx), "{fx:?}");
    assert!(!st.loading);
    cleanup(&[&linked, &dir]);
}

#[test]
fn an_auto_fetch_that_moved_a_ref_loads_everything() {
    let (mut st, dir, linked) = opened("fetch-refresh-moved");
    live(&mut st);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/x.git"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.snap.as_ref().unwrap().remotes.iter().all(|(_, branches)| branches.is_empty()), "no branch of origin yet");
    assert!(st.watch.live_for(&st.workdir().unwrap()), "still live");
    // What a fetch that brought a branch leaves behind.
    git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    let (fx, later) = auto_fetch_ends(&mut st);
    let msg = run_load(fx);
    assert!(matches!(msg, Msg::Repo(RepoMsg::RepoLoaded { kind: LoadKind::Refresh, result: Ok(_), .. })));
    update(&mut st, msg, later);
    let listed =
        st.snap.as_ref().unwrap().remotes.iter().any(|(remote, branches)| remote == "origin" && branches.iter().any(|(b, _)| b == "main"));
    assert!(listed, "the new remote branch is shown: {:?}", st.snap.as_ref().unwrap().remotes);
    cleanup(&[&linked, &dir]);
}

#[test]
fn without_a_live_watcher_the_status_and_worktrees_are_read_as_before() {
    let (mut st, dir, linked) = opened("fetch-refresh-no-probe");
    st.watch.probes.clear();
    let (fx, later) = auto_fetch_ends(&mut st);
    let msg = run_load(fx);
    assert!(matches!(msg, Msg::Load(LoadMsg::RepoUnchanged { .. })), "the status is read");
    let fx = update(&mut st, msg, later);
    assert!(counts(&fx), "the recount is due: {fx:?}");
    cleanup(&[&linked, &dir]);
}
