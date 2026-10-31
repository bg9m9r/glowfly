//! Headless tests for live working-tree updates: what a watcher report refreshes, when it is
//! ignored, which repository the watcher follows, and config edited outside Glowfly (new
//! remotes, settings that make git run programs, the identity commits get).

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
use crate::state::trust::Standing;
use crate::state::{AppState, Modal, RunningJob};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    st.job = Some(job(Some(glowfly_core::paths::temp_dir().join("glowfly-watch-some-other-repo"))));
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

fn index_changed(workdir: &Path, paths: &[&str], last_event: Instant) -> Msg {
    let msg = WatchMsg::IndexChanged { paths: paths.iter().map(|s| s.to_string()).collect(), last_event };
    Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(Msg::Watch(msg)) }
}

#[test]
fn an_index_change_reads_the_index() {
    use crate::state::refresh::Wanted;
    use glowfly_core::repo::FileEntry;
    let (mut st, dir, workdir) = opened("watch-index");
    let snap = st.snap.as_mut().unwrap();
    let old = snap.index_memo.clone().expect("a memo");
    let renamed = FileEntry { renamed_from: Some("old.txt".into()), ..FileEntry::new("new.txt", 'R') };
    snap.staged = vec![FileEntry::new("added.txt", 'A'), FileEntry::new("edited.txt", 'M'), renamed];
    let fx = update(&mut st, index_changed(&workdir, &["a.txt"], Instant::now()), Instant::now());
    match &fx[..] {
        [Effect::LoadIndexStatus { workdir: w, generation, read, .. }] => {
            assert!(w == &workdir && *generation == st.load_gen);
            assert!(std::sync::Arc::ptr_eq(&read.old, &old), "compared with the memo on screen");
            assert_eq!(read.paths, ["a.txt"]);
            assert_eq!(read.partners, ["added.txt", "new.txt", "old.txt"], "the rows a rename may pair with");
        }
        other => panic!("one index read expected: {other:?}"),
    }
    // While it runs, another index change queues, merging with a queued path read.
    st.refresh.status_queued = Some(Wanted::Paths(["b.txt".to_string()].into()));
    assert!(update(&mut st, index_changed(&workdir, &["c.txt"], Instant::now()), Instant::now()).is_empty());
    assert_eq!(st.refresh.status_queued, Some(Wanted::Index(["b.txt".to_string(), "c.txt".to_string()].into())));
    st.refresh.status_busy = false;
    st.refresh.status_queued = None;

    // Ignored while a job runs here, and for Glowfly's own changes.
    st.job = Some(job(Some(dir.clone())));
    assert!(update(&mut st, index_changed(&workdir, &[], Instant::now()), Instant::now()).is_empty());
    st.job = None;
    let t = Instant::now();
    st.watch.quiet_until = Some(t);
    assert!(update(&mut st, index_changed(&workdir, &[], t), t).is_empty());
    st.watch.quiet_until = None;

    // Without a memo of the index on screen: the whole status.
    st.snap.as_mut().unwrap().index_memo = None;
    let fx = update(&mut st, index_changed(&workdir, &[], Instant::now()), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadStatus { paths: None, .. }]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs `msg`, then every effect it returns, collecting all the effects along the way.
fn step(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    let mut all = Vec::new();
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for e in update(st, m, Instant::now()) {
            if matches!(e, Effect::LoadRepo { .. } | Effect::LoadStatus { .. } | Effect::LoadDiff { .. }) {
                queue.extend(runner::testing::execute(e));
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
    let msgs: Vec<Msg> = fx.into_iter().filter_map(runner::testing::execute).collect();
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

    // A trusted repository that gains a setting asks again, about that setting.
    let dir = scratch_repo("watch-trust-config-trusted");
    let mut st = state();
    crate::state::trust::record_as_found(&mut st.settings, &std::fs::canonicalize(&dir).unwrap());
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    // The decision took the first load's print, so the load options changed: load once with them.
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let workdir = st.workdir().unwrap();
    git(&dir, &["config", "core.sshCommand", "ssh -v"]);
    assert!(full_report(&mut st, &workdir));
    assert_eq!(st.trust_standing(), Some(Standing::Changed));
    assert!(st.held());
    match &st.modal {
        Some(Modal::Trust(p)) => assert!(p.gained.as_ref().is_some_and(|g| g.config.iter().any(|c| c.key == "core.sshCommand"))),
        _ => panic!("the trust prompt is open again"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_config_edit_outside_reads_the_identity_again() {
    let (mut st, dir, workdir) = opened("watch-identity");
    let config_changed = || Msg::Scoped { workdir: workdir.clone(), msg: Box::new(Msg::Watch(WatchMsg::ConfigChanged)) };
    let git_dir = st.snap.as_ref().unwrap().git_dir.clone();
    git(&dir, &["config", "user.name", "Grace Hopper"]);
    let mut fx = update(&mut st, config_changed(), Instant::now());
    let [Effect::Profile(crate::profile::ProfileEffect::ReadIdentity { git_dir: g, .. })] = &fx[..] else {
        panic!("one identity read: {fx:?}")
    };
    assert_eq!(g, &git_dir);
    // The read only reads git config; the repository's own name comes before any global one.
    let msg = runner::testing::execute(fx.remove(0)).expect("a result");
    let fx = update(&mut st, msg, Instant::now());
    finish(&mut st, fx);
    assert_eq!(st.identities[&git_dir].name, "Grace Hopper", "the edited config");

    st.settings.watch_files = false;
    assert!(update(&mut st, config_changed(), Instant::now()).is_empty(), "not while watching is off");
    let _ = std::fs::remove_dir_all(&dir);
}
