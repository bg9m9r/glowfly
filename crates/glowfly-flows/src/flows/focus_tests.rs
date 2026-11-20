//! End-to-end tests for returning to the window (update/focus.rs), through the runner: with the
//! watcher live and the repository just read, nothing runs (the identity check finds the config
//! files as they were and git isn't asked); an edited repository config reads the identity
//! again; once the last whole read is `FOCUS_FRESH` old the status is read as before.

use super::tests::{finish_keeping, git, scratch_repo, state};
use super::update;
use glowfly_model::effect::{Effect, LoadEffect, WorktreeEffect};
use glowfly_model::msg::{Msg, RepoMsg, WatchMsg};
use glowfly_model::profile::ProfileEffect;
use glowfly_model::state::AppState;
use glowfly_model::state::refresh::FOCUS_FRESH;
use glowfly_model::state::watch::Probe;
use std::path::PathBuf;
use std::time::Instant;

/// `scratch_repo(name)` opened in the only tab, its identity read (the test runner skips profile
/// effects, so the open's read is run here) and its watcher live.
fn opened_live(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    let msg = glowfly_runner::testing::execute(identity_read(kept)).expect("the first read asks git");
    update(&mut st, msg, Instant::now());
    let workdir = st.workdir().unwrap();
    update(&mut st, Msg::Watch(WatchMsg::Probed { workdir, probe: Probe::Live }), Instant::now());
    assert!(st.watch.live_for(&st.workdir().unwrap()), "the watcher is on the repository and live");
    (st, dir)
}

fn focus_at(st: &mut AppState, now: Instant) -> Vec<Effect> {
    update(st, Msg::Repo(RepoMsg::WindowFocused), now)
}

fn reads_status(fx: &[Effect]) -> bool {
    fx.iter().any(|e| {
        matches!(e, Effect::Load(LoadEffect::Repo { .. } | LoadEffect::Status { .. }) | Effect::Worktree(WorktreeEffect::CountChanges(_)))
    })
}

/// The one identity read among `fx`, taken out.
fn identity_read(fx: Vec<Effect>) -> Effect {
    let mut reads: Vec<_> = fx.into_iter().filter(|e| matches!(e, Effect::Profile(ProfileEffect::ReadIdentity { .. }))).collect();
    assert_eq!(reads.len(), 1, "one identity read: {reads:?}");
    reads.remove(0)
}

#[test]
fn focus_with_a_live_watcher_runs_no_git() {
    let (mut st, dir) = opened_live("focus-live");
    let fx = focus_at(&mut st, Instant::now());
    assert!(!reads_status(&fx), "{fx:?}");
    assert!(!st.loading);
    let read = identity_read(fx);
    assert!(glowfly_runner::testing::execute(read).is_none(), "the config files are as they were: git isn't asked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edited_repository_config_reads_the_identity_again() {
    let (mut st, dir) = opened_live("focus-config-edit");
    let git_dir = st.snap.as_ref().unwrap().git_dir.clone();
    let before = st.identities[&git_dir].stamp;
    assert!(before.is_some(), "the open's read stored its stamp");

    git(&dir, &["config", "user.name", "Grace Hopper"]);
    let fx = focus_at(&mut st, Instant::now());
    assert!(!reads_status(&fx), "{fx:?}");
    let msg = glowfly_runner::testing::execute(identity_read(fx)).expect("the config changed: git is asked");
    update(&mut st, msg, Instant::now());
    let id = &st.identities[&git_dir];
    assert_eq!(id.name, "Grace Hopper");
    assert!(id.stamp.is_some() && id.stamp != before, "stored with the new stamp");

    let fx = focus_at(&mut st, Instant::now());
    assert!(glowfly_runner::testing::execute(identity_read(fx)).is_none(), "unchanged again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn after_the_fresh_window_focus_refreshes() {
    let (mut st, dir) = opened_live("focus-stale-window");
    let fx = focus_at(&mut st, Instant::now() + FOCUS_FRESH);
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Repo { .. }))), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
