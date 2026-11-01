//! Headless tests for spacing the timed status reads of a repository on a network volume: the
//! next read is due from the end of the last one, a slow read stretches the wait (up to
//! `POLL_GAP_CAP`), no read is queued behind one already running, and focus still pauses and
//! resumes the timer.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, StatusRead};
use crate::msg::{LoadMsg, Msg, RepoMsg, WatchMsg};
use crate::state::AppState;
use crate::state::watch::{POLL_GAP_CAP, REMOTE_POLL};
use std::time::{Duration, Instant};

fn statuses(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadStatus { .. })).count()
}

/// A focused tab on a repository the runner said is on a network volume.
fn remote(name: &str) -> (AppState, std::path::PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.view.was_focused = true;
    let workdir = st.workdir().unwrap();
    st.watch.remote.insert(workdir, "SMB");
    (st, dir)
}

fn tick(st: &mut AppState, at: Instant) -> Vec<Effect> {
    update(st, Msg::Tick(at), at)
}

/// The running status read ends at `at`, with the status unchanged.
fn read_ends(st: &mut AppState, at: Instant) {
    let snap = st.snap.as_ref().unwrap();
    let status = StatusRead::Whole(snap.unstaged.clone(), snap.staged.clone(), snap.state);
    let generation = st.load_gen;
    let msg = LoadMsg::StatusLoaded {
        generation,
        result: Ok(status),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    };
    update(st, Msg::Load(msg), at);
    assert!(!st.refresh.status_busy);
}

/// Starts the clock at `t0` and lets the first timed read start: when it started.
fn first_read(st: &mut AppState, t0: Instant) -> Instant {
    assert_eq!(statuses(&tick(st, t0)), 0, "the first tick starts the clock");
    let started = t0 + REMOTE_POLL;
    assert_eq!(statuses(&tick(st, started)), 1, "due");
    started
}

/// Whether the next timed read is due exactly `gap` after `from`.
fn due_after(st: &mut AppState, from: Instant, gap: Duration) {
    assert_eq!(statuses(&tick(st, from + gap - Duration::from_millis(1))), 0, "not yet");
    assert_eq!(statuses(&tick(st, from + gap)), 1, "due {gap:?} after the read ended");
}

#[test]
fn the_next_read_is_spaced_from_the_end_of_the_last() {
    let (mut st, dir) = remote("poll-space-end");
    let started = first_read(&mut st, Instant::now());
    let ended = started + Duration::from_secs(2);
    read_ends(&mut st, ended);
    assert_eq!(st.watch.last_read, Some(Duration::from_secs(2)));
    due_after(&mut st, ended, REMOTE_POLL);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_slow_read_stretches_the_wait_to_three_times_its_length() {
    let (mut st, dir) = remote("poll-space-slow");
    let started = first_read(&mut st, Instant::now());
    let ended = started + Duration::from_secs(15);
    read_ends(&mut st, ended);
    due_after(&mut st, ended, Duration::from_secs(45));
    // A quick read afterwards brings the interval back.
    read_ends(&mut st, ended + Duration::from_secs(46));
    due_after(&mut st, ended + Duration::from_secs(46), REMOTE_POLL);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_stretch_is_capped() {
    let (mut st, dir) = remote("poll-space-cap");
    let started = first_read(&mut st, Instant::now());
    let ended = started + Duration::from_secs(300);
    read_ends(&mut st, ended);
    due_after(&mut st, ended, POLL_GAP_CAP);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_read_is_queued_behind_a_running_one() {
    let (mut st, dir) = remote("poll-space-busy");
    let started = first_read(&mut st, Instant::now());
    assert!(st.refresh.status_busy);
    for later in [REMOTE_POLL, POLL_GAP_CAP * 3] {
        assert_eq!(statuses(&tick(&mut st, started + later)), 0, "the read still runs");
        assert!(st.refresh.status_queued.is_none(), "nothing queued behind it");
    }
    let ended = started + POLL_GAP_CAP * 3;
    read_ends(&mut st, ended);
    due_after(&mut st, ended, POLL_GAP_CAP);

    // A read the timer didn't start (a watcher burst) runs when the timer is due: it isn't
    // doubled, and its end starts the clock again.
    let quick = ended + POLL_GAP_CAP;
    read_ends(&mut st, quick);
    let t = quick + REMOTE_POLL;
    let burst = WatchMsg::RepoChanged { full: false, paths: None, last_event: t };
    assert_eq!(statuses(&update(&mut st, Msg::Watch(burst), t)), 1, "the watcher's read");
    assert_eq!(statuses(&tick(&mut st, t)), 0);
    assert!(st.refresh.status_queued.is_none());
    read_ends(&mut st, t + Duration::from_secs(1));
    due_after(&mut st, t + Duration::from_secs(1), REMOTE_POLL);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_background_window_pauses_the_timer_and_focus_resumes_it() {
    let (mut st, dir) = remote("poll-space-focus");
    let started = first_read(&mut st, Instant::now());
    let ended = started + Duration::from_secs(1);
    read_ends(&mut st, ended);
    st.view.was_focused = false;
    assert_eq!(statuses(&tick(&mut st, ended + REMOTE_POLL * 5)), 0, "not in the background");
    st.view.was_focused = true;
    assert_eq!(statuses(&tick(&mut st, ended + REMOTE_POLL * 6)), 1, "overdue once focused again");
    let _ = std::fs::remove_dir_all(&dir);
}
