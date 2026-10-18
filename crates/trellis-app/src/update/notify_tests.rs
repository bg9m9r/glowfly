//! Headless tests for the "N commits behind" alert after a background fetch and the Pull
//! button on its toast (update/notify.rs).

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, finish, git, same_place};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{JobMsg, Msg, RepoMsg, TabsMsg};
use crate::runner;
use crate::state::lfs::LfsInstall;
use crate::state::notify::BehindWatch;
use crate::state::{AppState, ToastAction};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::gitcli::args;

fn behind(st: &AppState) -> usize {
    st.snap.as_ref().and_then(|s| s.head_info()).map(|h| h.behind).unwrap_or(usize::MAX)
}

fn action_toasts(st: &AppState) -> usize {
    st.toasts.iter().filter(|t| t.action.is_some()).count()
}

/// Runs the auto-fetch the next interval tick starts, to completion.
fn auto_fetch(st: &mut AppState) {
    st.settings.auto_fetch_minutes = 1;
    let tick = st.last_fetch + Duration::from_secs(61);
    drive(st, Msg::Tick(tick));
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn auto_fetch_that_puts_the_branch_behind_alerts() {
    let (mut st, work, bare) = with_origin("notify-behind");
    remote_commit(&bare, "main", "r1.txt");
    auto_fetch(&mut st);
    assert_eq!(behind(&st), 1);
    let t = st.toasts.last().expect("an alert");
    assert_eq!(t.text, "main is 1 commit behind origin/main");
    assert!(t.ok);
    let a = t.action.as_ref().expect("a Pull button");
    assert_eq!(a.label, "Pull");
    assert!(a.workdir.as_deref().is_some_and(|w| same_place(w, &work)));

    remote_commit(&bare, "main", "r2.txt");
    auto_fetch(&mut st);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("main is 2 commits behind origin/main"));
    assert_eq!(action_toasts(&st), 2);

    // Nothing new: the refresh finds the refs unchanged and says nothing.
    auto_fetch(&mut st);
    assert_eq!(st.toasts.len(), 2);
    assert!(st.behind_watch.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn manual_fetch_does_not_alert() {
    let (mut st, work, bare) = with_origin("notify-manual");
    remote_commit(&bare, "main", "r1.txt");
    drive(&mut st, Msg::git("Fetch", &["fetch", "--all"]));
    assert_eq!(behind(&st), 1, "the fetch happened");
    assert_eq!(action_toasts(&st), 0);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Fetch succeeded"));
    cleanup(&[&work, &bare]);
}

#[test]
fn no_alert_when_the_setting_is_off() {
    let (mut st, work, bare) = with_origin("notify-off");
    st.settings.behind_alerts = false;
    remote_commit(&bare, "main", "r1.txt");
    auto_fetch(&mut st);
    assert_eq!(behind(&st), 1);
    assert!(st.toasts.is_empty(), "auto-fetch is quiet");
    cleanup(&[&work, &bare]);
}

#[test]
fn no_alert_in_detached_head() {
    let (mut st, work, bare) = with_origin("notify-detached");
    git(&work, &["checkout", "-q", "--detach"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.snap.as_ref().is_some_and(|s| s.head_branch.is_none()));
    remote_commit(&bare, "main", "r1.txt");
    auto_fetch(&mut st);
    assert!(st.toasts.is_empty());
    cleanup(&[&work, &bare]);
}

#[test]
fn no_alert_for_a_tab_switched_away() {
    let (mut st, work, bare) = with_origin("notify-tabs");
    remote_commit(&bare, "main", "r1.txt");
    st.settings.auto_fetch_minutes = 1;
    let tick = st.last_fetch + Duration::from_secs(61);
    let mut fx = update(&mut st, Msg::Tick(tick), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]), "{fx:?}");
    let run = fx.remove(0);
    let fx = update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    finish(&mut st, fx);
    let done = runner::execute(run).expect("the job's result");
    let fx = update(&mut st, done, Instant::now());
    finish(&mut st, fx);
    assert!(st.behind_watch.is_none());
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(behind(&st), 1, "the fetch happened");
    assert_eq!(action_toasts(&st), 0);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_load_in_flight_keeps_the_watch_for_the_next_one() {
    let (mut st, work, bare) = with_origin("notify-inflight");
    let snap = st.snap.as_ref().unwrap();
    let w = BehindWatch {
        workdir: snap.workdir.clone(),
        branch: "main".into(),
        upstream: "origin/main".into(),
        behind: 0,
        after_gen: st.load_gen,
    };
    st.behind_watch = Some(w.clone());
    let reloaded = trellis_core::repo::load(&work, &st.load_options(&work)).unwrap();
    let generation = st.load_gen;
    update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Refresh, result: Ok(Box::new(reloaded)) }), Instant::now());
    assert_eq!(st.behind_watch, Some(w), "a load started before the fetch ended doesn't show its result");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.behind_watch.is_none(), "a later load takes it");
    assert_eq!(action_toasts(&st), 0, "still up to date");
    cleanup(&[&work, &bare]);
}

#[test]
fn pull_action_runs_pull() {
    let (mut st, work, bare) = with_origin("notify-pull");
    remote_commit(&bare, "main", "r1.txt");
    auto_fetch(&mut st);
    let id = st.toasts.last().expect("an alert").id;
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => assert_eq!(cmds, &vec![args(&["pull", "--ff"])]),
        other => panic!("expected a pull, got {other:?}"),
    }
    assert!(st.toasts.iter().all(|t| t.id != id));
    finish(&mut st, fx);
    assert_eq!(behind(&st), 0);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Pull succeeded"));
    cleanup(&[&work, &bare]);
}

#[test]
fn pull_action_for_another_tab_is_refused() {
    let (mut st, work, bare) = with_origin("notify-elsewhere");
    let action = ToastAction {
        label: "Pull".into(),
        msg: Box::new(Msg::git("Pull", &["pull", "--ff"])),
        workdir: Some(PathBuf::from("C:/somewhere/else")),
    };
    let now = Instant::now();
    let id = st.toast_action(true, "main is 1 commit behind origin/main", action, now);
    assert!(update(&mut st, Msg::ToastAction(id), now).is_empty());
    assert!(st.toasts.iter().all(|t| t.id != id));
    let last = st.toasts.last().expect("a refusal");
    assert!(!last.ok);
    assert_eq!(last.text, "Pull wasn't run: switch back to that repository's tab first.");
    assert!(st.job.is_none());
    // An unknown (expired) toast does nothing.
    let count = st.toasts.len();
    assert!(update(&mut st, Msg::ToastAction(9999), now).is_empty());
    assert_eq!(st.toasts.len(), count);
    cleanup(&[&work, &bare]);
}

/// An origin one commit ahead, in a repository that stores `*.bin` in LFS, with git-lfs
/// installed: a successful auto-fetch is followed by the LFS download.
fn lfs_behind(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    std::fs::write(work.join(".gitattributes"), "*.bin filter=lfs diff=lfs merge=lfs -text\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.lfs.install = LfsInstall::Installed("3.7.1".into());
    remote_commit(&bare, "main", "r1.txt");
    (st, work, bare)
}

fn is_lfs_fetch(e: &Effect) -> bool {
    matches!(e, Effect::RunGit { cmds, .. } if *cmds == [args(&["lfs", "fetch"])])
}

fn assert_alert(st: &AppState) {
    let t = st.toasts.iter().find(|t| t.action.is_some()).expect("the behind alert");
    assert_eq!(t.text, "main is 1 commit behind origin/main");
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("Pull"));
}

#[test]
fn a_failed_lfs_download_after_auto_fetch_keeps_the_behind_alert() {
    let (mut st, work, bare) = lfs_behind("notify-lfs-failed");
    st.settings.auto_fetch_minutes = 1;
    let tick = st.last_fetch + Duration::from_secs(61);
    let mut fx = update(&mut st, Msg::Tick(tick), Instant::now());
    let done = runner::execute(fx.remove(0)).expect("the fetch's result");
    let (lfs, mut rest): (Vec<_>, Vec<_>) = update(&mut st, done, Instant::now()).into_iter().partition(is_lfs_fetch);
    assert_eq!(lfs.len(), 1, "the LFS download started");
    let failed = Msg::Job(JobMsg::JobDone { ok: false, output: "$ git lfs fetch\nfailed".into(), undo: None });
    rest.extend(update(&mut st, failed, Instant::now()));
    finish(&mut st, rest);
    assert_alert(&st);
    assert!(st.log.iter().any(|l| l.text.starts_with("Auto-fetch LFS files FAILED")));
    assert!(st.toasts.iter().all(|t| t.ok), "a background failure only reaches the log");
    cleanup(&[&work, &bare]);
}

#[test]
fn an_lfs_download_ending_before_the_reload_keeps_the_alert() {
    // `finish` handles the newest result first, so the download ends before the reload lands.
    let (mut st, work, bare) = lfs_behind("notify-lfs-first");
    auto_fetch(&mut st);
    assert!(st.log.iter().any(|l| l.text.starts_with("Auto-fetch LFS files")), "the download ran");
    assert_alert(&st);
    cleanup(&[&work, &bare]);
}
