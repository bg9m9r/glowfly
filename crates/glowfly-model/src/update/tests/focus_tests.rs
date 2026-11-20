//! Headless tests for returning to the window (update/focus.rs): with the file watcher trusted
//! and the last whole status read recent, nothing is read again (the identity is only checked
//! against its config stamp, other worktrees follow their throttle); every other case refreshes
//! as before.

use super::tests::{scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadEffect, LoadKind, StatusRead, WorktreeEffect};
use crate::msg::{LoadMsg, Msg, PrefsMsg, RepoMsg, TabsMsg, WatchMsg};
use crate::profile::ProfileEffect;
use crate::state::refresh::FOCUS_FRESH;
use crate::state::refresh::recount::RECOUNT_EVERY;
use crate::state::watch::Probe;
use crate::state::{AppState, RunningJob, Tab};
use git2::RepositoryState;
use glowfly_core::config_stamp::{self, ConfigStamp};
use glowfly_core::error::ReadFailed;
use glowfly_core::repo::{self, ChangesRead, FileEntry, PathsStatus};
use glowfly_core::repo_identity::RepoIdentity;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What makes the watcher untrusted or the read stale.
type Spoil = fn(&mut AppState);

type Rows = (Vec<FileEntry>, Vec<FileEntry>, RepositoryState);

/// `scratch_repo(name)` loaded into the only tab, its identity stored with the current config
/// stamp, and a whole status read that ended at `read_at`.
fn opened(name: &str, read_at: Instant) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    st.snap = Some(repo::load(&dir, &st.load_options(&dir)).unwrap());
    let git_dir = st.snap.as_ref().unwrap().git_dir.clone();
    let stamp = Some(config_stamp::of_repo(&git_dir));
    st.identities.insert(git_dir, RepoIdentity { stamp, ..RepoIdentity::default() });
    st.refresh.status_read_at = Some(read_at);
    (st, dir)
}

/// The watcher is on the active repository and its probe answered `probe` (None: not yet).
fn watched(st: &mut AppState, probe: Option<Probe>) {
    let workdir = st.workdir().unwrap();
    st.watch.target = Some(workdir.clone());
    if let Some(p) = probe {
        st.watch.probes.insert(workdir, p);
    }
}

fn live(st: &mut AppState) {
    watched(st, Some(Probe::Live));
}

fn focus(st: &mut AppState, now: Instant) -> Vec<Effect> {
    update(st, Msg::Repo(RepoMsg::WindowFocused), now)
}

fn loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Load(LoadEffect::Repo { .. } | LoadEffect::Status { .. }))).count()
}

fn counts(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Worktree(WorktreeEffect::CountChanges(_)))).count()
}

/// The `known` of every identity read in `fx`.
fn identity_reads(fx: &[Effect]) -> Vec<Option<ConfigStamp>> {
    fx.iter().filter_map(|e| if let Effect::Profile(ProfileEffect::ReadIdentity { known, .. }) = e { Some(*known) } else { None }).collect()
}

/// Another worktree, counted before, that a recount would read again.
fn with_side(st: &mut AppState, dir: &Path) {
    let snap = st.snap.as_mut().unwrap();
    let mut side = snap.worktrees[0].clone();
    side.current = false;
    side.read = ChangesRead::Counted;
    side.path = dir.with_file_name("focus-side");
    snap.worktrees.push(side);
}

fn unchanged(st: &mut AppState, result: Result<Rows, ReadFailed>, now: Instant) {
    let msg = Msg::Load(LoadMsg::RepoUnchanged {
        generation: st.load_gen,
        result,
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    });
    update(st, msg, now);
}

fn status_loaded(st: &mut AppState, result: Result<StatusRead, ReadFailed>, now: Instant) {
    st.refresh.status_busy = true;
    let msg = Msg::Load(LoadMsg::StatusLoaded {
        generation: st.load_gen,
        result,
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    });
    update(st, msg, now);
}

fn clean() -> Rows {
    (vec![], vec![], RepositoryState::Clean)
}

#[test]
fn a_live_watcher_and_a_recent_read_read_nothing_again() {
    let t0 = Instant::now();
    let (mut st, dir) = opened("focus-fresh", t0);
    live(&mut st);
    let stamp = st.identities.values().next().unwrap().stamp;
    assert!(stamp.is_some());

    let fx = focus(&mut st, t0 + FOCUS_FRESH - Duration::from_millis(1));
    assert_eq!(loads(&fx), 0, "{fx:?}");
    assert!(!st.loading);
    assert_eq!(identity_reads(&fx), vec![stamp], "the identity is only checked against its stamp");

    // Other worktrees keep their interval: a count just made isn't repeated, one is kept due.
    with_side(&mut st, &dir);
    st.refresh.recount.started(t0);
    let fx = focus(&mut st, t0 + Duration::from_secs(1));
    assert_eq!((loads(&fx), counts(&fx)), (0, 0), "{fx:?}");
    assert_eq!(st.refresh.recount.due(), Some(t0 + RECOUNT_EVERY));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_read_refreshes() {
    let t0 = Instant::now();
    let (mut st, dir) = opened("focus-stale", t0);
    live(&mut st);
    let fx = focus(&mut st, t0 + FOCUS_FRESH);
    let [Effect::Load(LoadEffect::Repo { kind: LoadKind::Refresh, known, .. }), ..] = &fx[..] else { panic!("{fx:?}") };
    assert!(known.is_some(), "a checked refresh");
    assert_eq!(loads(&fx), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_trusted_watcher_focus_refreshes() {
    let t0 = Instant::now();
    let cases: [(&str, Spoil); 8] = [
        ("no read yet", |st| st.refresh.status_read_at = None),
        ("still checking", |st| {
            st.watch.probes.clear();
        }),
        ("silent", |st| watched(st, Some(Probe::Silent))),
        ("unknown", |st| watched(st, Some(Probe::Unknown))),
        ("failed", |st| {
            let w = st.workdir().unwrap();
            st.watch.down.insert(w);
        }),
        ("network volume", |st| {
            let w = st.workdir().unwrap();
            st.watch.remote.insert(w, "network");
        }),
        ("not watching", |st| st.settings.watch_files = false),
        ("history freed", |st| st.session.tab_view.history_freed = Some(1)),
    ];
    for (i, (what, spoil)) in cases.iter().enumerate() {
        let (mut st, dir) = opened(&format!("focus-spoiled-{i}"), t0);
        live(&mut st);
        spoil(&mut st);
        let fx = focus(&mut st, t0 + Duration::from_secs(1));
        assert_eq!(loads(&fx), 1, "{what}: {fx:?}");
        assert!(matches!(&fx[0], Effect::Load(LoadEffect::Repo { .. })), "{what}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_job_running_reads_nothing_on_focus() {
    let t0 = Instant::now();
    let (mut st, dir) = opened("focus-job", t0);
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_continue: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    let fx = focus(&mut st, t0 + Duration::from_secs(60));
    assert_eq!(loads(&fx), 0, "the job's end refreshes: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_reads_set_the_time_and_failures_clear_it() {
    let t0 = Instant::now();
    let (mut st, dir) = opened("focus-reads", t0);
    st.refresh.status_read_at = None;
    unchanged(&mut st, Ok(clean()), t0 + Duration::from_secs(1));
    assert_eq!(st.refresh.status_read_at, Some(t0 + Duration::from_secs(1)));

    let t2 = t0 + Duration::from_secs(2);
    status_loaded(&mut st, Ok(StatusRead::Whole(vec![], vec![], RepositoryState::Clean)), t2);
    assert_eq!(st.refresh.status_read_at, Some(t2), "a whole status read");

    let paths = PathsStatus { covered: vec![], unstaged: vec![], state: RepositoryState::Clean, submodule_touched: false };
    status_loaded(&mut st, Ok(StatusRead::Paths(paths)), t0 + Duration::from_secs(3));
    assert_eq!(st.refresh.status_read_at, Some(t2), "a read of some paths isn't a whole one");

    status_loaded(&mut st, Err(ReadFailed::NoFolder), t0 + Duration::from_secs(4));
    assert_eq!(st.refresh.status_read_at, None, "a failed status read");

    unchanged(&mut st, Ok(clean()), t0 + Duration::from_secs(5));
    assert!(st.refresh.status_read_at.is_some());
    unchanged(&mut st, Err(ReadFailed::NoFolder), t0 + Duration::from_secs(6));
    assert_eq!(st.refresh.status_read_at, None, "a failed unchanged read");

    // A load that read the status counts, and a failed one clears it.
    let snap = repo::load(&dir, &st.load_options(&dir)).unwrap();
    let t7 = t0 + Duration::from_secs(7);
    let loaded = |st: &AppState, result| Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result });
    let msg = loaded(&st, Ok(Box::new(snap)));
    update(&mut st, msg, t7);
    assert_eq!(st.refresh.status_read_at, Some(t7), "a load");
    let msg = loaded(&st, Err(ReadFailed::NoFolder));
    update(&mut st, msg, t7 + Duration::from_secs(1));
    assert_eq!(st.refresh.status_read_at, None, "a failed load");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_keeps_the_time_and_a_tab_switch_resets_it() {
    let t0 = Instant::now();
    let (mut st, dir) = opened("focus-keeps", t0);
    st.tabs.push(Tab::unloaded(dir.with_file_name("focus-keeps-other")));
    live(&mut st);
    let fx = focus(&mut st, t0 + FOCUS_FRESH);
    assert_eq!(loads(&fx), 1);
    assert_eq!(st.refresh.status_read_at, Some(t0), "starting a load keeps it");

    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), t0);
    assert_eq!(st.refresh.status_read_at, None, "another tab's repository");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn focus_checks_the_identity_and_other_triggers_force_it() {
    let t0 = Instant::now();
    let (mut st, dir) = opened("focus-identity", t0);
    live(&mut st);
    let stamp = st.identities.values().next().unwrap().stamp;
    assert_eq!(identity_reads(&focus(&mut st, t0)), vec![stamp]);

    let fx = update(&mut st, Msg::Watch(WatchMsg::ConfigChanged), t0);
    assert_eq!(identity_reads(&fx), vec![None], "a watched config change forces it");
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs), t0);
    assert_eq!(identity_reads(&fx), vec![None], "closing Preferences forces it");

    // Nothing stored (never read, or the last read failed): read in any case.
    st.identities.clear();
    assert_eq!(identity_reads(&focus(&mut st, t0)), vec![None]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_recount_due_on_focus_runs_when_the_status_is_skipped() {
    let t0 = Instant::now();
    let (mut st, dir) = opened("focus-recount", t0);
    live(&mut st);
    with_side(&mut st, &dir);
    st.refresh.recount.started(t0);
    let fx = focus(&mut st, t0 + RECOUNT_EVERY);
    assert_eq!(counts(&fx), 1, "{fx:?}");
    assert_eq!(loads(&fx), 0, "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
