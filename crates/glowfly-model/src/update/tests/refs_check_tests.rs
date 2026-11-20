//! Headless tests for the refresh after a background fetch (update/refs_check.rs): with the
//! file watcher live and nothing held back during the job, auto-fetch and its LFS download ask
//! only for a refs check (no status read, no worktree recount); every other job, and a fetch
//! with no live watcher or with a report held back, refreshes as before. The runner answers
//! the check in glowfly-runner; the flows in glowfly-flows run it end to end.

use super::tests::{long_repo, scratch_repo, state};
use super::update;
use crate::effect::{Effect, KnownRepo, LoadEffect, LoadKind, WorktreeEffect};
use crate::msg::{JobMsg, LoadMsg, Msg, RepoMsg, WatchMsg};
use crate::state::notify::{AUTO_FETCH, AUTO_FETCH_LFS};
use crate::state::refresh::Depth;
use crate::state::watch::Probe;
use crate::state::{AppState, RunningJob, Tab};
use git2::RepositoryState;
use glowfly_core::repo::{self, ChangesRead};
use std::path::PathBuf;
use std::time::Instant;

/// `scratch_repo(name)` loaded into the only tab, with the state's own load options (so a
/// refresh keeps what it knows of the snapshot).
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    st.snap = Some(repo::load(&dir, &st.load_options(&dir)).unwrap());
    (st, dir)
}

/// Another worktree, counted before, that a recount would read again.
fn with_side(st: &mut AppState, dir: &std::path::Path) {
    let snap = st.snap.as_mut().unwrap();
    let mut side = snap.worktrees[0].clone();
    side.current = false;
    side.read = ChangesRead::Counted;
    side.path = dir.with_file_name("refs-check-side");
    snap.worktrees.push(side);
}

/// The watcher is on the active repository and its probe saw events arrive.
fn live(st: &mut AppState) {
    let workdir = st.workdir().unwrap();
    st.watch.target = Some(workdir.clone());
    st.watch.probes.insert(workdir, Probe::Live);
}

fn job(st: &AppState, label: &str, quiet: bool) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir: st.workdir(),
        quiet,
        clear_commit: false,
        then_push: false,
        then_continue: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    }
}

/// A job named `label` ends at `now` in the active tab; the effects.
fn ended_at(st: &mut AppState, label: &str, quiet: bool, now: Instant) -> Vec<Effect> {
    st.job = Some(job(st, label, quiet));
    update(st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }), now)
}

fn ended(st: &mut AppState, label: &str, quiet: bool) -> Vec<Effect> {
    ended_at(st, label, quiet, Instant::now())
}

/// The `known` of the one repository load in `fx`.
fn known(fx: &[Effect]) -> Option<KnownRepo> {
    let mut loads = fx.iter().filter_map(|e| if let Effect::Load(LoadEffect::Repo { known, .. }) = e { Some(known) } else { None });
    let known = loads.next().expect("a repository load");
    assert!(loads.next().is_none(), "one load: {fx:?}");
    known.clone()
}

fn refs_only(fx: &[Effect]) -> bool {
    known(fx).is_some_and(|k| k.refs_only)
}

fn has_status(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Status { .. })))
}

fn counts(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Worktree(WorktreeEffect::CountChanges(_))))
}

fn unchanged(st: &mut AppState) -> Vec<Effect> {
    let msg = Msg::Load(LoadMsg::RepoUnchanged {
        generation: st.load_gen,
        result: Ok((vec![], vec![], RepositoryState::Clean)),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    });
    update(st, msg, Instant::now())
}

fn refs_unchanged(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::Load(LoadMsg::RefsUnchanged { generation: st.load_gen }), Instant::now())
}

/// A report of the watcher for the active tab.
fn report(st: &mut AppState, msg: WatchMsg) -> Vec<Effect> {
    let workdir = st.workdir().unwrap();
    update(st, Msg::Scoped { workdir, msg: Box::new(Msg::Watch(msg)) }, Instant::now())
}

#[test]
fn an_auto_fetch_with_a_live_watcher_checks_only_the_refs() {
    let (mut st, dir) = opened("refs-check-light");
    live(&mut st);
    with_side(&mut st, &dir);

    let fx = ended(&mut st, AUTO_FETCH, true);
    assert!(matches!(&fx[..], [Effect::Load(LoadEffect::Repo { kind: LoadKind::Refresh, .. })]), "{fx:?}");
    assert!(refs_only(&fx) && !has_status(&fx));
    assert_eq!(fx[0].name(), "check-refs");

    let fx = refs_unchanged(&mut st);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Load(_))), "{fx:?}");
    assert!(!counts(&fx), "no worktree recount");
    assert!(!st.loading);

    // The same state refreshed by a status-only answer does recount: the skip is real.
    let fx = ended(&mut st, AUTO_FETCH, true);
    assert!(refs_only(&fx));
    assert!(counts(&unchanged(&mut st)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn its_lfs_download_checks_only_the_refs_too() {
    let (mut st, dir) = opened("refs-check-lfs");
    live(&mut st);
    assert!(refs_only(&ended(&mut st, AUTO_FETCH_LFS, true)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_live_watcher_the_fetch_refreshes_as_before() {
    fn probe(st: &mut AppState, probe: Probe) {
        live(st);
        let workdir = st.workdir().unwrap();
        st.watch.probes.insert(workdir, probe);
    }
    type Setup = fn(&mut AppState);
    let setups: [(&str, Setup); 6] = [
        ("still checking", |_| {}),
        ("silent", |st| probe(st, Probe::Silent)),
        ("unknown", |st| probe(st, Probe::Unknown)),
        ("failed", |st| {
            live(st);
            let workdir = st.workdir().unwrap();
            update(st, Msg::Watch(WatchMsg::WatchFailed { workdir, error: "gone".into() }), Instant::now());
        }),
        ("watching off", |st| {
            live(st);
            st.settings.watch_files = false;
        }),
        ("another folder", |st| {
            let other = glowfly_core::paths::temp_dir().join("glowfly-refs-check-other");
            st.watch.target = Some(other.clone());
            st.watch.probes.insert(other, Probe::Live);
        }),
    ];
    for (i, (what, setup)) in setups.into_iter().enumerate() {
        let (mut st, dir) = opened(&format!("refs-check-dark-{i}"));
        setup(&mut st);
        with_side(&mut st, &dir);
        let fx = ended(&mut st, AUTO_FETCH, true);
        let k = known(&fx).expect("a known repository");
        assert!(!k.refs_only, "{what}");
        if i == 0 {
            assert!(counts(&unchanged(&mut st)), "{what}: the recount is asked for");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn other_jobs_refresh_in_full() {
    for (i, (label, quiet)) in
        [("Fetch", false), ("Pull", false), ("Auto-stash put-back", true), (AUTO_FETCH, false)].into_iter().enumerate()
    {
        let (mut st, dir) = opened(&format!("refs-check-other-jobs-{i}"));
        live(&mut st);
        let fx = ended(&mut st, label, quiet);
        assert!(!refs_only(&fx), "{label} (quiet: {quiet})");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_report_held_during_the_fetch_reads_the_status() {
    let now = Instant::now();
    type Report = fn(Instant) -> WatchMsg;
    let reports: [(&str, Report); 3] = [
        ("work tree", |t| WatchMsg::RepoChanged { full: false, paths: Some(vec!["a.txt".into()]), last_event: t }),
        ("index", |t| WatchMsg::IndexChanged { paths: vec!["a.txt".into()], last_event: t }),
        ("refs", |t| WatchMsg::RepoChanged { full: true, paths: None, last_event: t }),
    ];
    for (i, (what, make)) in reports.into_iter().enumerate() {
        let (mut st, dir) = opened(&format!("refs-check-held-{i}"));
        live(&mut st);
        st.job = Some(job(&st, AUTO_FETCH, true));
        let fx = report(&mut st, make(now));
        assert!(fx.is_empty(), "{what}: held back while the job runs: {fx:?}");
        assert!(st.watch.held, "{what}");

        let fx = ended(&mut st, AUTO_FETCH, true);
        assert!(!refs_only(&fx), "{what}: the job's end reads as before");
        assert!(!st.watch.held, "{what}: taken by the job's end");

        // The load ends, and the next fetch had no reports.
        unchanged(&mut st);
        let fx = ended(&mut st, AUTO_FETCH, true);
        assert!(refs_only(&fx), "{what}: nothing held this time");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn work_tree_reports_right_after_the_check_still_count() {
    let now = Instant::now();
    let (mut st, dir) = opened("refs-check-settle");
    live(&mut st);
    assert!(refs_only(&ended_at(&mut st, AUTO_FETCH, true, now)));
    let fx = report(&mut st, WatchMsg::RepoChanged { full: false, paths: None, last_event: now });
    assert!(has_status(&fx), "the user's edit: {fx:?}");
    let fx = report(&mut st, WatchMsg::RepoChanged { full: true, paths: None, last_event: now });
    assert!(fx.is_empty(), "the fetch's own refs, which the check covers: {fx:?}");

    // Contrast: after any other job every report in the settle window is dropped.
    let (mut st, dir2) = opened("refs-check-settle-full");
    live(&mut st);
    assert!(!refs_only(&ended_at(&mut st, "Fetch", false, now)));
    let fx = report(&mut st, WatchMsg::RepoChanged { full: false, paths: None, last_event: now });
    assert!(fx.is_empty(), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn a_check_asked_for_during_a_load_waits_and_merges() {
    let (mut st, dir) = opened("refs-check-queued");
    live(&mut st);
    let fx = update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert!(st.loading, "{fx:?}");
    let fx = ended(&mut st, AUTO_FETCH, true);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Repo { .. }))), "{fx:?}");
    assert_eq!(st.refresh.queued, Some(Depth::Refs));
    let fx = unchanged(&mut st);
    assert!(refs_only(&fx), "the queued check runs when the load ends");

    // A queued Checked refresh covers it.
    let (mut st, dir2) = opened("refs-check-queued-checked");
    live(&mut st);
    update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    update(&mut st, Msg::Repo(RepoMsg::WindowFocused), Instant::now());
    assert_eq!(st.refresh.queued, Some(Depth::Checked));
    ended(&mut st, AUTO_FETCH, true);
    assert_eq!(st.refresh.queued, Some(Depth::Checked));
    let fx = unchanged(&mut st);
    assert!(!refs_only(&fx), "the later, deeper request wins");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn a_status_read_dropped_by_the_check_is_read_after_it() {
    let (mut st, dir) = opened("refs-check-dropped");
    live(&mut st);
    let fx = report(&mut st, WatchMsg::RepoChanged { full: false, paths: None, last_event: Instant::now() });
    assert!(has_status(&fx) && st.refresh.status_busy, "{fx:?}");
    let fx = ended(&mut st, AUTO_FETCH, true);
    assert!(refs_only(&fx));
    assert!(st.refresh.status_after, "the check reads no status, so the dropped one is asked for after it");
    let fx = refs_unchanged(&mut st);
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Status { paths: None, .. }))), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_check_is_ignored() {
    let (mut st, dir) = opened("refs-check-stale");
    live(&mut st);
    assert!(refs_only(&ended(&mut st, AUTO_FETCH, true)));
    let old = st.load_gen;
    let fx = update(&mut st, Msg::Load(LoadMsg::RefsUnchanged { generation: old - 1 }), Instant::now());
    assert!(fx.is_empty() && st.loading, "{fx:?}");

    // F5 supersedes the check, and its answer comes too late.
    update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    assert!(st.load_gen > old);
    let fx = update(&mut st, Msg::Load(LoadMsg::RefsUnchanged { generation: old }), Instant::now());
    assert!(fx.is_empty() && st.loading, "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_raised_commit_limit_loads_more_instead() {
    let dir = long_repo("refs-check-more", 1200);
    let mut st = state();
    st.settings.initial_commits = 500;
    st.tabs[0] = Tab::unloaded(dir.clone());
    st.snap = Some(repo::load(&dir, &st.load_options(&dir)).unwrap());
    assert!(st.snap.as_ref().is_some_and(|s| s.frontier.is_some()), "cut short with a frontier");
    live(&mut st);
    st.refresh.extra_commits = 500;
    let fx = ended(&mut st, AUTO_FETCH, true);
    assert!(matches!(&fx[..], [Effect::Load(LoadEffect::Repo { kind: LoadKind::More, .. })]), "{fx:?}");
    assert!(!refs_only(&fx));
    let _ = std::fs::remove_dir_all(&dir);
}
