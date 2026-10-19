//! Refreshes that only walk the history when something besides the status changed, and that
//! don't pile up while one is running.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::JobMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::WatchMsg;
use crate::runner;
use crate::state::{AppState, RunningJob};
use std::path::PathBuf;
use std::time::Instant;

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

fn loads(fx: &[Effect]) -> Vec<&Effect> {
    fx.iter().filter(|e| matches!(e, Effect::LoadRepo { .. })).collect()
}

/// The one load in `fx`, run: the message it reports.
fn run_load(fx: Vec<Effect>) -> Msg {
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { .. })).expect("a load");
    runner::execute(load).expect("a result")
}

fn quiet_job(dir: &std::path::Path) -> RunningJob {
    RunningJob {
        label: "Auto-fetch".into(),
        workdir: Some(dir.to_path_buf()),
        quiet: true,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    }
}

#[test]
fn a_job_that_changed_no_refs_reads_only_the_status() {
    let (mut st, dir) = opened("refresh-quiet");
    st.job = Some(quiet_job(&st.workdir().unwrap()));
    let fx = update(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }), Instant::now());
    let msg = run_load(fx);
    assert!(matches!(msg, Msg::Load(LoadMsg::RepoUnchanged { .. })), "an auto-fetch that brought nothing doesn't walk the history");
    let fx = update(&mut st, msg, Instant::now());
    assert!(!st.loading);
    assert!(loads(&fx).is_empty(), "{fx:?}");

    // A new commit (or any ref change) loads everything.
    git(&dir, &["commit", "-q", "-am", "second"]);
    let fx = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    let msg = run_load(fx);
    assert!(matches!(msg, Msg::Repo(RepoMsg::RepoLoaded { kind: LoadKind::Refresh, result: Ok(_), .. })));
    update(&mut st, msg, Instant::now());
    assert_eq!(st.snap.as_ref().unwrap().commits[0].summary, "second");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_and_load_option_changes_load_everything() {
    let (mut st, dir) = opened("refresh-config");
    git(&dir, &["remote", "add", "x", "https://example.invalid/x.git"]);
    let msg = run_load(update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now()));
    assert!(matches!(msg, Msg::Repo(RepoMsg::RepoLoaded { .. })), "a remote was added");
    update(&mut st, msg, Instant::now());

    st.settings.initial_commits += 100;
    let fx = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert!(matches!(loads(&fx)[..], [Effect::LoadRepo { known: None, .. }]), "the commit limit changed: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refreshes_wait_for_the_running_load_and_merge() {
    let (mut st, dir) = opened("refresh-merge");
    let first = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert_eq!(loads(&first).len(), 1);
    for _ in 0..3 {
        let fx = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
        assert!(loads(&fx).is_empty(), "no second load while one runs: {fx:?}");
    }
    let fx = update(&mut st, run_load(first), Instant::now());
    assert_eq!(loads(&fx).len(), 1, "one more load for the three requests: {fx:?}");
    let fx = update(&mut st, run_load(fx), Instant::now());
    assert!(loads(&fx).is_empty(), "and no more after it: {fx:?}");
    assert!(!st.loading);

    // F5 doesn't wait: it supersedes the load in flight.
    let first = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert!(matches!(loads(&fx)[..], [Effect::LoadRepo { known: None, .. }]), "{fx:?}");
    let stale = run_load(first);
    assert!(update(&mut st, stale, Instant::now()).is_empty(), "the superseded result is dropped");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn status_reads_merge_and_a_new_change_relayouts_the_graph() {
    let dir = scratch_repo("refresh-status");
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.as_ref().unwrap().graph.wip.is_none(), "clean");

    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    let workdir = st.workdir().unwrap();
    let changed = || Msg::Scoped {
        workdir: workdir.clone(),
        msg: Box::new(Msg::Watch(WatchMsg::RepoChanged { full: false, paths: None, last_event: Instant::now() })),
    };
    let first = update(&mut st, changed(), Instant::now());
    assert!(matches!(&first[..], [Effect::LoadStatus { .. }]), "{first:?}");
    assert!(update(&mut st, changed(), Instant::now()).is_empty(), "waits for the read running");
    assert!(update(&mut st, changed(), Instant::now()).is_empty());

    let status = first.into_iter().next().and_then(runner::execute).unwrap();
    let fx = update(&mut st, status, Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadStatus { .. }]), "one more read for both: {fx:?}");
    assert!(loads(&fx).is_empty());
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.is_dirty() && snap.graph.wip.is_some(), "the WIP row appears without a reload");
    finish(&mut st, fx);
    assert!(!st.refresh.status_busy && st.refresh.status_queued.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_watcher_rescan_checks_refs_before_loading() {
    let (mut st, dir) = opened("refresh-rescan");
    let workdir = st.workdir().unwrap();
    let msg =
        Msg::Scoped { workdir, msg: Box::new(Msg::Watch(WatchMsg::RepoChanged { full: true, paths: None, last_event: Instant::now() })) };
    let fx = update(&mut st, msg, Instant::now());
    assert!(matches!(loads(&fx)[..], [Effect::LoadRepo { known: Some(_), .. }]), "{fx:?}");
    assert!(matches!(run_load(fx), Msg::Load(LoadMsg::RepoUnchanged { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

fn diff_loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadDiff { .. })).count()
}

/// Asks for the status and runs the read: the effects its result leads to.
fn read_status(st: &mut AppState) -> Vec<Effect> {
    let workdir = st.workdir().unwrap();
    let changed =
        Msg::Scoped { workdir, msg: Box::new(Msg::Watch(WatchMsg::RepoChanged { full: false, paths: None, last_event: Instant::now() })) };
    let fx = update(st, changed, Instant::now());
    let status = fx.into_iter().find(|e| matches!(e, Effect::LoadStatus { .. })).and_then(runner::execute).expect("a status read");
    update(st, status, Instant::now())
}

#[test]
fn a_status_read_rereads_the_open_diff_only_when_its_file_changed() {
    let (mut st, dir) = opened("refresh-diff");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(trellis_core::repo::DiffTarget::Unstaged("a.txt".into()))));
    assert!(st.diff.is_some());
    // The first read after opening has nothing to compare with.
    let fx = read_status(&mut st);
    finish(&mut st, fx);
    assert_eq!(diff_loads(&read_status(&mut st)), 0, "nothing changed");

    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let fx = read_status(&mut st);
    assert_eq!(diff_loads(&fx), 1, "the file changed: {fx:?}");
    finish(&mut st, fx);
    assert_eq!(st.diff.as_ref().unwrap().1.hunks[0].lines.iter().filter(|l| l.origin == '+').count(), 2);

    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    assert_eq!(diff_loads(&read_status(&mut st)), 1, "the index changed");

    let head = st.snap.as_ref().unwrap().commits[0].oid;
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(trellis_core::repo::DiffTarget::Commit(head, "a.txt".into()))));
    std::fs::write(dir.join("a.txt"), "other\n").unwrap();
    assert_eq!(diff_loads(&read_status(&mut st)), 0, "a commit's diff doesn't change");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_more_raises_the_limit_until_the_tab_is_switched() {
    let (mut st, dir) = opened("refresh-more");
    let limit = st.settings.commit_limit();
    assert!(update(&mut st, Msg::Load(LoadMsg::LoadMoreCommits), Instant::now()).is_empty(), "the whole history is shown already");
    st.snap.as_mut().unwrap().truncated = true;
    let fx = update(&mut st, Msg::Load(LoadMsg::LoadMoreCommits), Instant::now());
    let [Effect::LoadRepo { opts, known: None, .. }] = &loads(&fx)[..] else { panic!("a full load: {fx:?}") };
    assert_eq!(opts.max_commits, 2 * limit);
    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().loaded_with.max_commits, 2 * limit);
    assert_eq!(st.load_options(&dir).max_commits, 2 * limit, "later refreshes keep it");

    let other = scratch_repo("refresh-more-other");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    assert_eq!(st.load_options(&other).max_commits, limit, "another tab starts at the limit");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}
