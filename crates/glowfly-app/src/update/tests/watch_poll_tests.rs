//! Headless tests for the watcher's safety nets: the notice for a repository on a network or
//! virtual file system, and checking again on a timer where the watcher can be silent (a sweep of
//! the tree on Windows, the whole status on a network volume).

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, WatchMsg};
use crate::state::AppState;
use crate::state::watch::{REMOTE_POLL, WINDOWS_POLL};
use std::time::{Duration, Instant};

fn statuses(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadStatus { .. })).count()
}

fn opened(name: &str) -> (AppState, std::path::PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.view.was_focused = true;
    (st, dir)
}

#[test]
fn a_remote_repository_is_announced_once_and_read_on_a_timer() {
    let (mut st, dir) = opened("watch-remote");
    let workdir = st.workdir().unwrap();
    let remote = || Msg::Watch(WatchMsg::RemoteVolume { workdir: workdir.clone(), kind: "SMB" });
    update(&mut st, remote(), Instant::now());
    let toast = st.toasts.last().expect("a notice").text.clone();
    assert!(toast.contains("SMB") && toast.contains("F5") && toast.contains("10 seconds"), "{toast}");
    let n = st.toasts.len();
    update(&mut st, remote(), Instant::now());
    assert_eq!(st.toasts.len(), n, "said once");
    assert_eq!(st.watch.poll_interval(), Some(REMOTE_POLL));

    let t0 = Instant::now();
    assert_eq!(statuses(&update(&mut st, Msg::Tick(t0), t0)), 0, "the first tick starts the clock");
    let t1 = t0 + REMOTE_POLL - Duration::from_millis(1);
    assert_eq!(statuses(&update(&mut st, Msg::Tick(t1), t1)), 0);
    let t2 = t0 + REMOTE_POLL;
    assert_eq!(statuses(&update(&mut st, Msg::Tick(t2), t2)), 1, "re-read when due");
    // Not while the window is in the background: regaining focus refreshes anyway.
    st.view.was_focused = false;
    let t3 = t2 + REMOTE_POLL * 2;
    assert_eq!(statuses(&update(&mut st, Msg::Tick(t3), t3)), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn windows_rereads_a_local_repository_too_and_nothing_polls_when_watching_is_off() {
    let (mut st, dir) = opened("watch-poll-local");
    assert_eq!(st.watch.poll_interval(), if cfg!(windows) { Some(WINDOWS_POLL) } else { None });
    st.settings.watch_files = false;
    super::watch::sync(&mut st, &mut Vec::new());
    assert_eq!(st.watch.poll_interval(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

fn sweeps(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SweepWatch { .. })).count()
}

/// The effects of a tick when the poll is due.
fn due_tick(st: &mut AppState) -> Vec<Effect> {
    let t0 = Instant::now();
    update(st, Msg::Tick(t0), t0);
    let t1 = t0 + WINDOWS_POLL;
    update(st, Msg::Tick(t1), t1)
}

#[test]
fn windows_sweeps_a_local_repository_instead_of_reading_it() {
    let (mut st, dir) = opened("watch-poll-sweep");
    let workdir = st.workdir().unwrap();
    let fx = due_tick(&mut st);
    if !cfg!(windows) {
        assert!(fx.is_empty(), "no poll elsewhere: {fx:?}");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    assert!(matches!(&fx[..], [Effect::SweepWatch { workdir: w }] if *w == workdir), "{fx:?}");
    assert_eq!(statuses(&fx), 0);

    st.watch.failed.insert(workdir.clone());
    st.watch.last_poll = None;
    let fx = due_tick(&mut st);
    assert_eq!((sweeps(&fx), statuses(&fx)), (0, 1), "a failed watcher: the whole status");
    st.refresh.status_busy = false;
    st.watch.failed.clear();

    st.watch.remote.insert(workdir, "SMB");
    st.watch.last_poll = None;
    let t0 = Instant::now();
    update(&mut st, Msg::Tick(t0), t0);
    let t1 = t0 + REMOTE_POLL;
    let fx = update(&mut st, Msg::Tick(t1), t1);
    assert_eq!((sweeps(&fx), statuses(&fx)), (0, 1), "a network volume: the whole status");
    let _ = std::fs::remove_dir_all(&dir);
}
