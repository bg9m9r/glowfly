//! Headless tests for the watcher's safety nets: the notice for a repository on a network or
//! virtual file system, checking again on a timer where the watcher can be silent (a sweep of
//! the tree on Windows, the whole status on a network volume), and what the watcher's probe
//! found (a silent watcher warned about once and polled, the status bar's dot).

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, WatchMsg};
use crate::state::AppState;
use crate::state::watch::{LiveUpdates, Probe, REMOTE_POLL, WINDOWS_POLL};
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

fn probed(workdir: &std::path::Path, probe: Probe) -> Msg {
    Msg::Watch(WatchMsg::Probed { workdir: workdir.to_path_buf(), probe })
}

#[test]
fn a_silent_watcher_is_warned_about_once_and_polled_until_events_arrive() {
    let (mut st, dir) = opened("watch-probe-silent");
    let workdir = st.workdir().unwrap();
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Checking), "the probe is out");
    let n = st.toasts.len();
    assert!(update(&mut st, probed(&workdir, Probe::Silent), Instant::now()).is_empty());
    assert_eq!(st.toasts.len(), n + 1);
    let toast = &st.toasts.last().unwrap();
    assert!(!toast.ok);
    let shown = glowfly_core::native_path::shown(&workdir).to_string();
    assert!(toast.text.contains(&shown) && toast.text.contains("10 seconds") && toast.text.contains("Restarting"), "{}", toast.text);
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == toast.text), "logged quietly");
    assert!(!st.view.show_log);
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Silent));
    assert_eq!(st.watch.poll_interval(), Some(REMOTE_POLL), "polled like a network volume");
    let t0 = Instant::now();
    update(&mut st, Msg::Tick(t0), t0);
    let fx = update(&mut st, Msg::Tick(t0 + REMOTE_POLL), t0 + REMOTE_POLL);
    assert_eq!(statuses(&fx) + sweeps(&fx), 1, "re-read when due: {fx:?}");

    // Its tab selected again: a new watcher, silent again, isn't announced twice.
    update(&mut st, probed(&workdir, Probe::Silent), Instant::now());
    assert_eq!(st.toasts.len(), n + 1, "warned once per repository");
    // A late event shows events do arrive: back to the usual safety net.
    update(&mut st, probed(&workdir, Probe::Live), Instant::now());
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Live));
    assert_eq!(st.watch.poll_interval(), if cfg!(windows) { Some(WINDOWS_POLL) } else { None });
    // Then the watcher fails: the dot says so.
    update(&mut st, Msg::Watch(WatchMsg::WatchFailed { workdir: workdir.clone(), error: "gone".into() }), Instant::now());
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Failed));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_live_or_unknown_probe_and_a_silent_network_volume_say_nothing_more() {
    let (mut st, dir) = opened("watch-probe-quiet");
    let workdir = st.workdir().unwrap();
    let n = st.toasts.len();
    update(&mut st, probed(&workdir, Probe::Live), Instant::now());
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Live));
    update(&mut st, probed(&workdir, Probe::Unknown), Instant::now());
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Unknown), "a read-only repository");
    assert_eq!(st.toasts.len(), n);
    update(&mut st, Msg::Watch(WatchMsg::RemoteVolume { workdir: workdir.clone(), kind: "WSL" }), Instant::now());
    let n = st.toasts.len();
    update(&mut st, probed(&workdir, Probe::Silent), Instant::now());
    assert_eq!(st.toasts.len(), n, "the network-volume notice already said why");
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Silent));
    st.settings.watch_files = false;
    super::watch::sync(&mut st, &mut Vec::new());
    assert_eq!(st.watch.live_updates(), None, "watching off: no dot");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restarted_watcher_checks_again() {
    let (mut st, dir) = opened("watch-probe-restart");
    let workdir = st.workdir().unwrap();
    let restart = |st: &mut AppState| {
        st.settings.watch_files = false;
        super::watch::sync(st, &mut Vec::new());
        st.settings.watch_files = true;
        let mut fx = Vec::new();
        super::watch::sync(st, &mut fx);
        assert!(matches!(&fx[..], [Effect::Watch(Some(_))]), "{fx:?}");
    };
    update(&mut st, probed(&workdir, Probe::Silent), Instant::now());
    assert_eq!(st.watch.poll_interval(), Some(REMOTE_POLL));
    restart(&mut st);
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Checking), "the new watcher's probe is out");
    assert_eq!(st.watch.poll_interval(), if cfg!(windows) { Some(WINDOWS_POLL) } else { None }, "an old Silent no longer polls");
    update(&mut st, Msg::Watch(WatchMsg::WatchFailed { workdir: workdir.clone(), error: "gone".into() }), Instant::now());
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Failed));
    restart(&mut st);
    assert_eq!(st.watch.live_updates(), Some(LiveUpdates::Checking), "a retried watcher isn't failed yet");
    let _ = std::fs::remove_dir_all(&dir);
}
