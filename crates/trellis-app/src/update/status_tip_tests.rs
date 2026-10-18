//! The note after a slow status read: when it shows, what it offers, that it shows once per
//! repository, and the job its button starts.

use super::note_text;
use crate::effect::{Effect, LoadKind, StatusRead};
use crate::msg::{LoadMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::update::tests::{drive, scratch_repo, state};
use crate::update::update;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::git_version::GitInfo;
use trellis_core::repo::{self, SlowStatus, StatusSpeedups};

const TOOK: Duration = Duration::from_millis(2400);

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn speedups(untracked_cache: bool, fsmonitor: bool) -> StatusSpeedups {
    StatusSpeedups { fsmonitor, untracked_cache, show_untracked_all: None }
}

/// A whole status read of the active repository that took `TOOK`, with `s` set.
fn slow_read(st: &AppState, s: StatusSpeedups) -> Msg {
    let snap = st.snap.as_ref().unwrap();
    let result = Ok(StatusRead::Whole(snap.unstaged.clone(), snap.staged.clone(), snap.state));
    let slow = Some(SlowStatus { took: TOOK, speedups: s });
    Msg::Load(LoadMsg::StatusLoaded { generation: st.load_gen, result, submodules: None, open_stamp: None, slow, lfs_patterns: None })
}

/// Hands `slow_read` to `update`: the effects it returns.
fn read_slowly(st: &mut AppState, s: StatusSpeedups) -> Vec<Effect> {
    let msg = slow_read(st, s);
    update(st, msg, Instant::now())
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

fn marked(st: &AppState) -> bool {
    st.settings.status_tip_shown_for(&st.snap.as_ref().unwrap().real_workdir)
}

#[test]
fn a_slow_read_offers_the_untracked_cache_once() {
    let (mut st, dir) = opened("tip-offer");
    let n = st.toasts.len();
    let fx = read_slowly(&mut st, speedups(false, false));
    assert_eq!(st.toasts.len(), n + 1);
    let toast = st.toasts.last().unwrap();
    assert!(
        toast.text.contains("2.4 s") && toast.text.contains("untracked cache") && toast.text.contains("core.fsmonitor"),
        "{}",
        toast.text
    );
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Turn on"));
    assert_eq!(saves(&fx), 1);
    assert!(marked(&st));
    assert!(st.log.iter().any(|l| l.text.contains("2.4 s")), "logged too");

    let fx = read_slowly(&mut st, speedups(false, false));
    assert_eq!(st.toasts.len(), n + 1, "never twice");
    assert_eq!(saves(&fx), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_note_mentions_only_what_is_off() {
    let (mut st, dir) = opened("tip-fsmonitor");
    let n = st.toasts.len();
    read_slowly(&mut st, speedups(true, false));
    let toast = st.toasts.last().unwrap();
    assert_eq!(st.toasts.len(), n + 1);
    assert!(toast.action.is_none() && toast.text.contains("core.fsmonitor") && !toast.text.contains("untracked"), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = opened("tip-nothing");
    let n = st.toasts.len();
    let fx = read_slowly(&mut st, speedups(true, true));
    assert_eq!(st.toasts.len(), n, "both on: nothing to say");
    assert!(marked(&st) && saves(&fx) == 1, "but never checked again");
    let _ = std::fs::remove_dir_all(&dir);

    // status.showUntrackedFiles=normal keeps the cache from helping Trellis's read.
    let (mut st, dir) = opened("tip-normal");
    let s = StatusSpeedups { show_untracked_all: Some(false), ..speedups(false, false) };
    read_slowly(&mut st, s);
    assert!(st.toasts.last().unwrap().action.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_held_repository_gets_no_note() {
    let (mut st, dir) = crate::update::test_mods::trust_held_tests::held_repo("tip-held");
    let n = st.toasts.len();
    let fx = read_slowly(&mut st, speedups(false, false));
    assert_eq!(st.toasts.len(), n);
    assert_eq!(saves(&fx), 0);
    assert!(!marked(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_older_git_gets_no_button() {
    let (mut st, dir) = opened("tip-old-git");
    st.git.info = Some(GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 36, 1)), chosen: false });
    read_slowly(&mut st, speedups(false, false));
    let toast = st.toasts.last().unwrap();
    assert!(toast.action.is_none() && toast.text.contains("core.fsmonitor") && !toast.text.contains("untracked"), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_slow_load_checks_too() {
    let (mut st, dir) = opened("tip-load");
    let mut snap = repo::load(&dir, &st.load_options(&dir)).unwrap();
    snap.slow_status = Some(SlowStatus { took: TOOK, speedups: speedups(false, false) });
    let n = st.toasts.len();
    let msg = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) });
    st.loading = true;
    update(&mut st, msg, Instant::now());
    assert_eq!(st.toasts.len(), n + 1);
    assert!(marked(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turn_on_starts_the_job_in_the_repository() {
    let (mut st, dir) = opened("tip-turn-on");
    let s = speedups(false, false);
    read_slowly(&mut st, s);
    let id = st.toasts.last().unwrap().id;
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    let workdir = st.workdir().unwrap();
    assert!(matches!(&fx[..], [Effect::RunGit { cwd, cmds, .. }] if cwd == &workdir && cmds == &repo::untracked_cache_cmds(s)), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Turn on the untracked cache"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn turn_on_does_nothing_from_another_tab() {
    let (mut st, dir) = opened("tip-other-tab");
    read_slowly(&mut st, speedups(false, false));
    let toast = st.toasts.last_mut().unwrap();
    let id = toast.id;
    // The note was for a repository that is no longer the active tab.
    toast.action.as_mut().unwrap().workdir = Some(dir.with_file_name("trellis-tip-elsewhere"));
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.job.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_note_reads_naturally() {
    let unset = speedups(false, false);
    assert_eq!(
        note_text(TOOK, unset, true),
        "Reading this repository's changes took 2.4 s. Git's untracked cache can speed this up; turning it on also makes git status \
         in a terminal list every untracked file rather than their folders. On Windows and macOS, git's built-in file system monitor \
         (core.fsmonitor) can help further."
    );
    let all = StatusSpeedups { show_untracked_all: Some(true), ..unset };
    assert!(!note_text(TOOK, all, true).contains("terminal"), "already listed one by one");
    assert_eq!(
        note_text(Duration::from_millis(1549), speedups(true, false), false),
        "Reading this repository's changes took 1.5 s. On Windows and macOS, git's built-in file system monitor (core.fsmonitor) can \
         speed this up."
    );
}
