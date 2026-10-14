//! Headless tests for the SUBMODULES section: opening a submodule in a tab, refusing what can't
//! be done, initializing and updating one as a job (asking first when out of sync), and keeping
//! the undo history across an update.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::Msg;
use crate::runner;
use crate::state::submodules::update_args;
use crate::state::tabs::same_path;
use crate::state::{AppState, Modal, RunningJob};
use crate::submodule::SubmoduleMsg;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{SubmoduleInfo, SubmoduleState};
use trellis_core::undo::Action;

const PATH: &str = "libs/lib";

/// A repository with a submodule at libs/lib (added from a sibling repository and committed),
/// opened in `st`. Returns (state, parent, sibling library repository).
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let lib = scratch_repo(&format!("{name}-lib"));
    let parent = scratch_repo(name);
    let url = format!("../{}", lib.file_name().unwrap().to_string_lossy());
    git(&parent, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, PATH]);
    git(&parent, &["commit", "-q", "-m", "add lib"]);
    let sub = parent.join(PATH);
    git(&sub, &["config", "user.name", "Test"]);
    git(&sub, &["config", "user.email", "test@example.com"]);
    git(&sub, &["config", "commit.gpgsign", "false"]);
    let mut st = state();
    drive(&mut st, Msg::Open(parent.clone()));
    (st, parent, lib)
}

fn sub(st: &AppState) -> SubmoduleInfo {
    st.snap.as_ref().expect("a repository is open").submodules.first().cloned().expect("a submodule")
}

fn msg(path: &str, confirmed: bool) -> Msg {
    Msg::Submodule(SubmoduleMsg::Update { path: path.into(), confirmed })
}

fn open(path: &str) -> Msg {
    Msg::Submodule(SubmoduleMsg::Open(path.into()))
}

/// Runs the loads and git jobs in `fx` (and everything they lead to) to completion; the
/// window, watcher and saving effects are skipped as in `drive`.
fn finish(st: &mut AppState, fx: Vec<Effect>) {
    for e in fx.into_iter().filter(|e| matches!(e, Effect::LoadRepo { .. } | Effect::RunGit { .. })) {
        if let Some(m) = runner::execute(e) {
            drive(st, m);
        }
    }
}

fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("a log entry")
}

fn deinit(st: &mut AppState, parent: &Path) {
    git(parent, &["submodule", "deinit", "-q", "-f", PATH]);
    drive(st, Msg::Refresh);
    assert_eq!(sub(st).state, SubmoduleState::Uninitialized);
}

fn commit_inside(st: &mut AppState, parent: &Path) {
    let dir = parent.join(PATH);
    std::fs::write(dir.join("a.txt"), "inside\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "inside"]);
    drive(st, Msg::Refresh);
    assert_eq!(sub(st).state, SubmoduleState::OutOfSync);
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn open_submodule_in_new_tab() {
    let (mut st, parent, lib) = opened("sm-open");
    let want = parent.join("libs").join("lib");
    let fx = update(&mut st, open(PATH), Instant::now());
    // Switching to the new tab parks the parent (watcher, title, saved tabs), then it loads.
    let loads: Vec<&Effect> = fx.iter().filter(|e| matches!(e, Effect::LoadRepo { .. })).collect();
    match &loads[..] {
        [Effect::LoadRepo { kind: LoadKind::Open, path, .. }] => assert!(same_path(path, &want), "{}", path.display()),
        other => panic!("{other:?}"),
    }
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    finish(&mut st, fx);
    assert!(same_path(&st.snap.as_ref().unwrap().workdir, &want));

    // Opening it again from the parent selects its tab.
    drive(&mut st, Msg::SelectTab(0));
    drive(&mut st, open(PATH));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    cleanup(&[&parent, &lib]);
}

#[test]
fn open_uninitialized_is_refused() {
    let (mut st, parent, lib) = opened("sm-open-uninit");
    deinit(&mut st, &parent);
    assert!(update(&mut st, open(PATH), Instant::now()).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains("Initialize it first"), "{text}");
    assert_eq!(st.tabs.len(), 1);

    assert!(update(&mut st, open("libs/gone"), Instant::now()).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains("no longer in this repository"), "{text}");
    cleanup(&[&parent, &lib]);
}

#[test]
fn initialize_runs_job_and_refreshes() {
    let (mut st, parent, lib) = opened("sm-init");
    deinit(&mut st, &parent);
    let fx = update(&mut st, msg(PATH, false), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { cmds, prompts: true, .. }] if *cmds == vec![update_args(PATH)]), "{fx:?}");
    assert!(st.modal.is_none());
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Initialize submodule libs/lib"));
    finish(&mut st, fx);
    let t = st.toasts.last().expect("a toast");
    assert!(t.ok && t.text == "Initialize submodule libs/lib succeeded", "{}", t.text);
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    cleanup(&[&parent, &lib]);
}

#[test]
fn out_of_sync_update_asks_first() {
    let (mut st, parent, lib) = opened("sm-update");
    commit_inside(&mut st, &parent);
    assert!(update(&mut st, msg(PATH, false), Instant::now()).is_empty());
    match &st.modal {
        Some(Modal::Confirm { then, title, .. }) => {
            assert_eq!(title, "Update submodule");
            assert!(matches!(&**then, Msg::Submodule(SubmoduleMsg::Update { confirmed: true, path }) if path == PATH));
        }
        _ => panic!("expected a confirmation"),
    }
    drive(&mut st, Msg::ConfirmAccepted);
    let sm = sub(&st);
    assert_eq!(sm.state, SubmoduleState::InSync);
    assert_eq!(sm.checked_out, Some(sm.recorded));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Update submodule libs/lib succeeded"));
    cleanup(&[&parent, &lib]);
}

#[test]
fn update_blocked_cases() {
    let (mut st, parent, lib) = opened("sm-blocked");
    let set = |st: &mut AppState, f: &dyn Fn(&mut SubmoduleInfo)| f(&mut st.snap.as_mut().unwrap().submodules[0]);

    set(&mut st, &|s| s.url = None);
    assert!(update(&mut st, msg(PATH, false), Instant::now()).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains(".gitmodules has no entry"), "{text}");

    set(&mut st, &|s| s.url = Some("../x".into()));
    assert!(update(&mut st, msg(PATH, true), Instant::now()).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.contains("Already checked out"), "{text}");

    set(&mut st, &|s| s.state = SubmoduleState::OutOfSync);
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: true,
    });
    assert!(update(&mut st, msg(PATH, true), Instant::now()).is_empty());
    assert_eq!(last_log(&st), (false, "Another operation is still running.".to_owned()));
    st.job = None;
    cleanup(&[&parent, &lib]);
}

#[test]
fn submodule_update_keeps_undo_history() {
    let (mut st, parent, lib) = opened("sm-undo");
    commit_inside(&mut st, &parent);
    st.undo.record(Action::DeleteBranch { name: "b".into(), oid: Oid::zero(), upstream: None });
    drive(&mut st, msg(PATH, true));
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    assert_eq!(st.undo.undo.len(), 1, "updating a submodule leaves the parent's history valid");
    assert!(st.undo.blocked.is_none());
    cleanup(&[&parent, &lib]);
}
