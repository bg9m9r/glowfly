//! Finished jobs as runs in the activity log: their title, summary and duration, a failure
//! opened with its fix shared by the panel and the toast, quiet auto-fetches, and folding.

use super::auto_stash_tests::git_cmds;
use super::tests::{clean_tree, drive, scratch_repo, state};
use crate::effect::Effect;
use crate::logs::LogsMsg;
use crate::msg::{JobMsg, Msg, RepoMsg, TabsMsg, TrustMsg};
use crate::state::AppState;
use crate::state::activity_rows::routine_ok;
use crate::state::logs::{EntryId, LogEntry};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::gitcli::args;
use trellis_core::logs::RunInfo;

fn at(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Effect> {
    crate::update::update(st, msg, now)
}

fn done(st: &mut AppState, ok: bool, output: &str, now: Instant) -> Vec<Effect> {
    at(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }), now)
}

/// A clean repository, opened.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn runs_git(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::RunGit { .. }))
}

fn last(st: &AppState) -> &LogEntry {
    st.log.last().expect("a log entry")
}

#[test]
fn job_becomes_a_run() {
    let (mut st, dir) = opened("runs-push");
    st.view.now = 1_800_000_000;
    let t0 = Instant::now();
    at(&mut st, Msg::git("Push", &["push"]), t0);
    assert!(st.job.is_some());
    done(&mut st, true, "$ git push\nEverything up-to-date", t0 + Duration::from_millis(1250));
    let e = last(&st);
    let want = RunInfo { title: "Push".into(), summary: "Everything up-to-date".into(), took_ms: Some(1250), routine: false };
    assert_eq!(e.run.as_ref(), Some(&want));
    assert_eq!(e.at, 1_800_000_000);
    assert_eq!(e.text, "Push done\n$ git push\nEverything up-to-date", "the text is unchanged");
    assert!(!st.logs.open.contains(&e.id), "a success starts closed");
    assert_eq!(st.logs.pending.last().and_then(|l| l.run.as_ref()), Some(&want), "the file gets the run too");
    assert_eq!(st.logs.job_started, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stamp_tracks_the_active_job() {
    let (mut st, dir) = opened("runs-stamp");
    let t0 = Instant::now();
    at(&mut st, Msg::git("Push", &["push"]), t0);
    assert_eq!(st.logs.job_started, Some(t0));
    at(&mut st, Msg::Tick(t0), t0 + Duration::from_secs(3));
    assert_eq!(st.logs.job_started, Some(t0), "kept over later updates");
    done(&mut st, true, "", t0 + Duration::from_secs(4));
    assert_eq!(st.logs.job_started, None);
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = super::trust_held_tests::held_repo("runs-stamp-held");
    at(&mut st, Msg::git("Push", &["push"]), t0);
    assert!(st.trust.held.is_some() && st.job.is_none(), "held for the trust prompt");
    assert_eq!(st.logs.job_started, None);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    let released = t0 + Duration::from_secs(9);
    at(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }), released);
    assert!(st.job.is_some());
    assert_eq!(st.logs.job_started, Some(released), "timed from its release");
    let _ = std::fs::remove_dir_all(&dir);
}

const LOCAL_CHANGES: &str = "$ git pull --ff\nerror: Your local changes to the following files would be overwritten by merge:\n\tf.txt\nPlease commit your changes or stash them before you merge.\nAborting";

/// A Pull that failed over local changes the snapshot hadn't shown; returns the run's id.
fn failed_pull(st: &mut AppState) -> EntryId {
    let now = Instant::now();
    at(st, Msg::git("Pull", &["pull", "--ff"]), now);
    assert!(st.job.is_some());
    done(st, false, LOCAL_CHANGES, now);
    last(st).id
}

#[test]
fn failed_run_opens_with_its_fix() {
    let (mut st, dir) = opened("runs-fix");
    let id = failed_pull(&mut st);
    let e = last(&st);
    let run = e.run.as_ref().expect("a run");
    assert_eq!((run.title.as_str(), run.summary.as_str()), ("Pull", "Your changes to 1 file would be overwritten."));
    assert!(st.logs.open.contains(&id), "a loud failure opens");
    assert_eq!(st.logs.fix_label(id), Some("Stash and pull"));
    let t = st.toasts.last().expect("the failure toast");
    let action = t.action.as_ref().expect("the fix");
    assert_eq!(action.label, "Stash and pull");
    assert!(matches!(*action.msg, Msg::Logs(LogsMsg::Fix(f)) if f == id), "the toast's button is the proxy");

    let fx = at(&mut st, Msg::Logs(LogsMsg::Fix(id)), Instant::now());
    assert_eq!(git_cmds(&fx).last(), Some(&args(&["pull", "--ff"])), "{fx:?}");
    assert_eq!(git_cmds(&fx).first().map(|c| c[0].as_str()), Some("stash"), "stashes first");
    assert_eq!(st.logs.fix(id).map(|_| ()), None, "the fix runs once");
    assert!(st.toasts.iter().all(|t| t.action.as_ref().is_none_or(|a| a.label != "Stash and pull")), "its toast is gone");
    st.job = None;
    let fx = at(&mut st, Msg::Logs(LogsMsg::Fix(id)), Instant::now());
    assert!(!runs_git(&fx), "a second Fix does nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fix_waits_for_its_tab() {
    let (mut st, dir) = opened("runs-fix-tab");
    let id = failed_pull(&mut st);
    at(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    st.toasts.clear();
    let fx = at(&mut st, Msg::Logs(LogsMsg::Fix(id)), Instant::now());
    assert!(!runs_git(&fx));
    let t = st.toasts.last().expect("a toast");
    assert_eq!(t.text, "Stash and pull wasn't run: switch back to that repository's tab first.");
    assert_eq!(st.logs.fix_label(id), Some("Stash and pull"), "the fix is kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn auto_fetch_stays_quiet() {
    let (mut st, work, bare) = super::branch_tests::with_origin("runs-auto");
    st.settings.auto_fetch_minutes = 1;
    let tick = st.last_fetch + Duration::from_secs(61);
    at(&mut st, Msg::Tick(tick), tick);
    let job = st.job.as_ref().expect("the auto-fetch runs");
    assert!(job.quiet);
    st.toasts.clear();
    done(&mut st, true, "$ git fetch --all", tick);
    assert!(st.toasts.is_empty(), "a successful background job never toasts");
    let e = last(&st);
    assert!(routine_ok(e), "{:?}", e.run);
    assert!(!st.logs.open.contains(&e.id));

    st.settings.auto_fetch_minutes = 1;
    let tick = st.last_fetch + Duration::from_secs(61);
    at(&mut st, Msg::Tick(tick), tick);
    assert!(st.job.is_some(), "the auto-fetch runs again");
    done(&mut st, false, "$ git fetch --all\nfatal: unable to access the remote", tick);
    assert!(st.toasts.is_empty(), "nor does a failed one");
    let e = last(&st);
    assert!(!routine_ok(e) && e.run.as_ref().is_some_and(|r| r.routine));
    assert!(!st.logs.open.contains(&e.id), "a background failure starts closed");
    for d in [&work, &bare] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn fold_toggles() {
    let mut st = state();
    let id = EntryId::next();
    at(&mut st, Msg::Logs(LogsMsg::Fold(id)), Instant::now());
    assert!(st.logs.open.contains(&id));
    at(&mut st, Msg::Logs(LogsMsg::Fold(id)), Instant::now());
    assert!(!st.logs.open.contains(&id));
}
