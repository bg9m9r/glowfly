//! Headless tests for live working-tree updates: what a watcher report refreshes, when it is
//! ignored, which repository the watcher follows, and config edited outside Trellis (new
//! remotes, settings that make git run programs).

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind, WatchTarget};
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::TabsMsg;
use crate::msg::WatchMsg;
use crate::runner;
use crate::state::trust::{Standing, record};
use crate::state::{AppState, Modal, RunningJob};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::settings::RepoTrust;

fn changed(workdir: &Path, full: bool, last_event: Instant) -> Msg {
    Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(Msg::Watch(WatchMsg::RepoChanged { full, paths: None, last_event })) }
}

fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let workdir = st.workdir().expect("opened");
    (st, dir, workdir)
}

fn job(workdir: Option<PathBuf>) -> RunningJob {
    RunningJob {
        label: "Fetch".into(),
        workdir,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    }
}

fn is_status(fx: &[Effect], st: &AppState, workdir: &Path) -> bool {
    matches!(fx, [Effect::LoadStatus { workdir: w, generation, .. }] if w == workdir && *generation == st.load_gen)
}

fn watches(fx: &[Effect]) -> Vec<Option<WatchTarget>> {
    fx.iter().filter_map(|e| if let Effect::Watch(t) = e { Some(t.clone()) } else { None }).collect()
}

fn watched_dir(fx: &[Effect]) -> Option<Option<PathBuf>> {
    match &watches(fx)[..] {
        [t] => Some(t.as_ref().map(|t| t.workdir.clone())),
        [] => None,
        more => panic!("several watch effects: {more:?}"),
    }
}

#[test]
fn outside_file_change_refreshes_status_only() {
    let (mut st, dir, workdir) = opened("watch-status");
    let fx = update(&mut st, changed(&workdir, false, Instant::now()), Instant::now());
    assert!(is_status(&fx, &st, &workdir), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ref_change_refreshes_everything() {
    let (mut st, dir, workdir) = opened("watch-full");
    let fx = update(&mut st, changed(&workdir, true, Instant::now()), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadRepo { kind: LoadKind::Refresh, path, .. }] if *path == workdir), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changes_for_another_tab_are_dropped() {
    let (a, b) = (scratch_repo("watch-tabs-a"), scratch_repo("watch-tabs-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    let a_workdir = st.workdir().unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    let b_workdir = st.workdir().unwrap();
    assert!(update(&mut st, changed(&a_workdir, false, Instant::now()), Instant::now()).is_empty());
    let fx = update(&mut st, changed(&b_workdir, false, Instant::now()), Instant::now());
    assert!(is_status(&fx, &st, &b_workdir), "{fx:?}");
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn own_job_suppresses_watch_events() {
    let (mut st, dir, workdir) = opened("watch-own-job");
    st.job = Some(job(Some(dir.clone())));
    assert!(update(&mut st, changed(&workdir, false, Instant::now()), Instant::now()).is_empty(), "a job runs here");
    st.job = Some(job(Some(trellis_core::paths::temp_dir().join("trellis-watch-some-other-repo"))));
    let fx = update(&mut st, changed(&workdir, false, Instant::now()), Instant::now());
    assert!(is_status(&fx, &st, &workdir), "a job elsewhere doesn't suppress: {fx:?}");
    finish(&mut st, fx);

    // Events from before the job finished (plus a grace period) were the job's own.
    st.job = Some(job(Some(workdir.clone())));
    let t = Instant::now();
    let fx = update(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }), t);
    finish(&mut st, fx);
    assert!(update(&mut st, changed(&workdir, false, t + Duration::from_millis(100)), t).is_empty());
    let fx = update(&mut st, changed(&workdir, false, t + Duration::from_millis(300)), t);
    assert!(is_status(&fx, &st, &workdir), "{fx:?}");
    finish(&mut st, fx);

    // The same after a local operation (staging).
    let t = Instant::now() + Duration::from_secs(5);
    let fx = update(&mut st, Msg::Scoped { workdir: workdir.clone(), msg: Box::new(Msg::Stage(StageMsg::LocalOpDone(Ok(None)))) }, t);
    finish(&mut st, fx);
    assert!(update(&mut st, changed(&workdir, false, t + Duration::from_millis(100)), t).is_empty());
    let fx = update(&mut st, changed(&workdir, false, t + Duration::from_millis(300)), t);
    assert!(is_status(&fx, &st, &workdir), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs `msg`, then every effect it returns, collecting all the effects along the way.
fn step(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    let mut all = Vec::new();
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for e in update(st, m, Instant::now()) {
            if matches!(e, Effect::LoadRepo { .. } | Effect::LoadStatus { .. } | Effect::LoadDiff { .. }) {
                queue.extend(runner::execute(e));
            } else {
                all.push(e);
            }
        }
    }
    all
}

#[test]
fn watcher_follows_the_active_tab() {
    let (a, b) = (scratch_repo("watch-follow-a"), scratch_repo("watch-follow-b"));
    let mut st = state();
    let fx = step(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    let snap = st.snap.as_ref().unwrap();
    let a_target = WatchTarget { workdir: snap.workdir.clone(), git_dir: snap.git_dir.clone() };
    assert_eq!(watches(&fx), [Some(a_target.clone())], "opening starts the watcher once loaded");
    assert_eq!(watched_dir(&step(&mut st, Msg::Repo(RepoMsg::Refresh))), None, "a refresh keeps it");

    step(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    let b_workdir = st.workdir().unwrap();
    assert_eq!(watched_dir(&update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now())), Some(Some(a_target.workdir.clone())));
    assert_eq!(watched_dir(&update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now())), Some(None), "an empty tab stops it");
    // Closing the active (empty) tab of three selects b's.
    assert_eq!(watched_dir(&update(&mut st, Msg::Tabs(TabsMsg::CloseTab(usize::MAX)), Instant::now())), Some(Some(b_workdir)));
    // Closing the active tab of two selects the survivor right away.
    assert_eq!(watched_dir(&update(&mut st, Msg::Tabs(TabsMsg::CloseTab(usize::MAX)), Instant::now())), Some(Some(a_target.workdir)));
    assert_eq!(
        watched_dir(&update(&mut st, Msg::Tabs(TabsMsg::CloseTab(usize::MAX)), Instant::now())),
        Some(None),
        "closing the last tab stops it"
    );
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn preference_toggle_starts_and_stops_watching() {
    let (mut st, dir, workdir) = opened("watch-pref");
    assert!(update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now()).is_empty(), "nothing changed");
    st.settings.watch_files = false;
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    assert!(matches!(&fx[..], [Effect::SaveSettings(_), Effect::Watch(None)]), "{fx:?}");
    assert!(update(&mut st, changed(&workdir, false, Instant::now()), Instant::now()).is_empty(), "ignored while off");
    st.settings.watch_files = true;
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now());
    assert!(matches!(&fx[..], [Effect::SaveSettings(_), Effect::Watch(Some(t))] if t.workdir == workdir), "{fx:?}");
    assert!(update(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited), Instant::now()).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn watch_failure_is_reported_once_per_repository() {
    let mut st = state();
    let failed = |w: &str| Msg::Watch(WatchMsg::WatchFailed { workdir: PathBuf::from(w), error: "too many watches".into() });
    assert!(update(&mut st, failed("C:/repo"), Instant::now()).is_empty());
    assert!(update(&mut st, failed("C:/repo"), Instant::now()).is_empty());
    assert_eq!(st.toasts.len(), 1);
    assert!(!st.toasts[0].ok);
    assert_eq!(st.log.len(), 1);
    assert!(!st.log[0].ok && st.log[0].text.contains("too many watches"));
    assert!(!st.view.show_log, "the activity log doesn't pop open");
    assert!(update(&mut st, failed("C:/other"), Instant::now()).is_empty());
    assert_eq!(st.toasts.len(), 2);
}

/// Delivers a full watcher report for `workdir` and runs what follows. Returns whether the
/// refresh walked everything again (not `RepoUnchanged`).
fn full_report(st: &mut AppState, workdir: &Path) -> bool {
    let fx = update(st, changed(workdir, true, Instant::now()), Instant::now());
    let [Effect::LoadRepo { kind: LoadKind::Refresh, known: Some(_), .. }] = &fx[..] else { panic!("a checked refresh: {fx:?}") };
    let msgs: Vec<Msg> = fx.into_iter().filter_map(runner::execute).collect();
    let full = msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::RepoLoaded { .. })));
    for m in msgs {
        let fx = update(st, m, Instant::now());
        finish(st, fx);
    }
    full
}

#[test]
fn a_remote_added_outside_reloads_the_remotes() {
    let (mut st, dir, workdir) = opened("watch-remote-added");
    git(&dir, &["remote", "add", "upstream", "https://example.com/u.git"]);
    assert!(full_report(&mut st, &workdir), "the config changed: a full load");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.remotes.iter().any(|(r, _)| r == "upstream"), "{:?}", snap.remotes);
    assert!(snap.remote_urls.contains(&("upstream".into(), "https://example.com/u.git".into())), "{:?}", snap.remote_urls);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn outside_config_that_runs_programs_asks_for_trust() {
    let (mut st, dir, workdir) = opened("watch-trust-config");
    assert_eq!(st.trust_standing(), Some(Standing::Clean));
    git(&dir, &["config", "core.sshCommand", "ssh -v"]);
    assert!(full_report(&mut st, &workdir));
    assert_eq!(st.trust_standing(), Some(Standing::Undecided));
    assert!(st.held(), "git is held");
    match &st.modal {
        Some(Modal::Trust(p)) => assert!(p.scan.config.iter().any(|c| c.key == "core.sshCommand"), "{:?}", p.scan),
        _ => panic!("the trust prompt is open"),
    }
    let _ = std::fs::remove_dir_all(&dir);

    // A trusted repository stays trusted.
    let dir = scratch_repo("watch-trust-config-trusted");
    let mut st = state();
    record(&mut st.settings, &std::fs::canonicalize(&dir).unwrap(), RepoTrust::Trusted);
    record(&mut st.settings, &dir, RepoTrust::Trusted);
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let workdir = st.workdir().unwrap();
    git(&dir, &["config", "core.sshCommand", "ssh -v"]);
    assert!(full_report(&mut st, &workdir));
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    assert!(!st.held() && !matches!(st.modal, Some(Modal::Trust(_))));
    let _ = std::fs::remove_dir_all(&dir);
}
