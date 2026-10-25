//! The slow-status note's file system monitor: what it offers on each OS and git, the job its
//! button starts, and what that job's end says.

use super::tests::{check_on, marked, opened, speedups};
use crate::effect::Effect;
use crate::effect::MonitorJob;
use crate::msg::{LoadMsg, Msg};
use crate::state::AppState;
use crate::update::update;
use glowfly_core::git_version::GitInfo;
use glowfly_core::opens::Os;
use glowfly_core::repo::{self, MonitorOutcome, MonitorTrouble, StatusSpeedups};
use std::time::Instant;

fn with_git(st: &mut AppState, version: (u32, u32, u32)) {
    st.git.info = Some(GitInfo { path: Some("/usr/bin/git".into()), version: Some(version), chosen: false });
}

/// The newest toast's text and the (cache, monitor) its Turn on asks for, if it has one.
fn note(st: &AppState) -> (String, Option<(bool, bool)>) {
    let toast = st.toasts.last().expect("a note");
    let offer = toast.action.as_ref().map(|a| match a.msg.as_ref() {
        Msg::Load(LoadMsg::SpeedUpStatus { cache, monitor, .. }) => (*cache, *monitor),
        _ => panic!("not the note's action"),
    });
    (toast.text.clone(), offer)
}

#[test]
fn windows_and_macos_offer_the_monitor() {
    for os in [Os::Windows, Os::MacOs] {
        let (mut st, dir) = opened("tip-monitor-offer");
        with_git(&mut st, (2, 45, 0));
        check_on(&mut st, speedups(true, false), os);
        let (text, offer) = note(&st);
        assert_eq!(offer, Some((false, true)), "{os:?}");
        assert!(text.contains("file system monitor") && text.contains("background") && text.contains("2.4 s"), "{text}");
        assert!(marked(&st));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn both_are_offered_in_one_button() {
    let (mut st, dir) = opened("tip-monitor-both");
    with_git(&mut st, (2, 45, 0));
    check_on(&mut st, speedups(false, false), Os::Windows);
    let (text, offer) = note(&st);
    assert_eq!(offer, Some((true, true)));
    assert!(text.contains("untracked cache") && text.contains("file system monitor") && text.contains("terminal"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn linux_suggests_watchman() {
    let (mut st, dir) = opened("tip-watchman");
    with_git(&mut st, (2, 45, 0));
    check_on(&mut st, speedups(true, false), Os::Linux);
    let (text, offer) = note(&st);
    assert_eq!(offer, None, "text only");
    assert!(text.contains("Watchman") && text.contains("fsmonitor-watchman"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = opened("tip-watchman-cache");
    with_git(&mut st, (2, 45, 0));
    check_on(&mut st, speedups(false, false), Os::Linux);
    let (text, offer) = note(&st);
    assert_eq!(offer, Some((true, false)));
    assert!(text.contains("Watchman"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_older_git_names_the_version_it_needs() {
    let (mut st, dir) = opened("tip-monitor-old");
    with_git(&mut st, (2, 35, 1));
    check_on(&mut st, speedups(false, false), Os::Windows);
    let (text, offer) = note(&st);
    assert_eq!(offer, None, "too old for the cache too");
    assert!(text.contains("2.36") && text.contains("2.35.1"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_network_drive_gets_no_monitor_button() {
    for (s, want) in [(speedups(true, false), None), (speedups(false, false), Some((true, false)))] {
        let (mut st, dir) = opened("tip-monitor-remote");
        with_git(&mut st, (2, 45, 0));
        let workdir = st.workdir().unwrap();
        st.watch.remote.insert(workdir, "SMB");
        check_on(&mut st, s, Os::Windows);
        let (text, offer) = note(&st);
        assert_eq!(offer, want);
        assert!(text.contains("network drives"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Sends the note's Turn on with these offers: the effects `update` returns.
fn turn_on(st: &mut AppState, s: StatusSpeedups, cache: bool) -> Vec<Effect> {
    update(st, Msg::Load(LoadMsg::SpeedUpStatus { speedups: s, cache, monitor: true }), Instant::now())
}

#[test]
fn turn_on_starts_the_monitor_job() {
    let s = speedups(false, false);
    let (mut st, dir) = opened("tip-monitor-job");
    let workdir = st.workdir().unwrap();
    let fx = turn_on(&mut st, s, false);
    assert!(matches!(&fx[..], [Effect::TurnOnMonitor(MonitorJob { cwd, then })] if cwd == &workdir && then.is_empty()), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Turn on the file system monitor"));
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = opened("tip-monitor-job-cache");
    let fx = turn_on(&mut st, s, true);
    assert!(matches!(&fx[..], [Effect::TurnOnMonitor(MonitorJob { then, .. })] if then == &repo::untracked_cache_cmds(s)), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Turn on the untracked cache and file system monitor"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Starts the monitor job and ends it with this result.
fn ended(st: &mut AppState, output: &str, monitor: MonitorOutcome, cache: Option<bool>) {
    turn_on(st, speedups(false, false), cache.is_some());
    assert!(st.job.is_some());
    update(st, Msg::Load(LoadMsg::StatusSpedUp { output: output.into(), monitor, cache }), Instant::now());
    assert!(st.job.is_none(), "the job ended");
}

#[test]
fn a_monitor_that_isnt_running_says_why() {
    let (mut st, dir) = opened("tip-monitor-remote-job");
    let output = "$ git fsmonitor--daemon status\nerror: remote repository '//srv/r' is incompatible with fsmonitor";
    ended(&mut st, output, MonitorOutcome::Trouble(MonitorTrouble::Remote), None);
    let toast = st.toasts.last().unwrap();
    assert!(!toast.ok);
    assert_eq!(toast.text, MonitorTrouble::Remote.text());
    let run = st.log.iter().rev().find(|l| l.run.is_some()).expect("a run");
    assert!(!run.ok && run.text.contains("FAILED") && run.text.contains("incompatible with fsmonitor"), "{}", run.text);
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = opened("tip-monitor-not-watching");
    ended(&mut st, "$ git fsmonitor--daemon status", MonitorOutcome::Trouble(MonitorTrouble::NotWatching), Some(true));
    let toast = st.toasts.last().unwrap();
    assert!(!toast.ok && toast.text.starts_with("The untracked cache is on."), "{}", toast.text);
    assert!(toast.text.ends_with(&MonitorTrouble::NotWatching.text()), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_monitor_on_says_so() {
    let (mut st, dir) = opened("tip-monitor-on");
    ended(&mut st, "$ git fsmonitor--daemon start", MonitorOutcome::Watching { started: true }, Some(true));
    let toast = st.toasts.last().unwrap();
    assert!(toast.ok);
    assert_eq!(toast.text, "Git's untracked cache and file system monitor are on.");
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = opened("tip-monitor-on-alone");
    ended(&mut st, "$ git fsmonitor--daemon status", MonitorOutcome::Watching { started: false }, None);
    let toast = st.toasts.last().unwrap();
    assert!(toast.ok);
    assert_eq!(toast.text, "Git's file system monitor is on and watching this repository.");
    let _ = std::fs::remove_dir_all(&dir);
}
