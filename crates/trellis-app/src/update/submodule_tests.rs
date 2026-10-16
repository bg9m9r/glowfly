//! Headless tests for the SUBMODULES section: opening a submodule in a tab, refusing what can't
//! be done, initializing and updating one as a job (asking first when out of sync, naming the
//! commits on no branch that it would leave behind), and keeping the undo history across an update.

use super::tests::same_place;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind, SubmoduleEffect};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::TabsMsg;
use crate::runner;
use crate::state::submodules::update_args;
use crate::state::{AppState, Modal, RunningJob};
use crate::submodule::SubmoduleMsg;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{SubmoduleInfo, SubmoduleState};
use trellis_core::undo::Action;

pub(super) const PATH: &str = "libs/lib";

/// A repository with a submodule at libs/lib (added from a sibling repository and committed),
/// opened in `st`. Returns (state, parent, sibling library repository).
pub(super) fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
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
    drive(&mut st, Msg::Repo(RepoMsg::Open(parent.clone())));
    (st, parent, lib)
}

pub(super) fn sub(st: &AppState) -> SubmoduleInfo {
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

pub(super) fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("a log entry")
}

fn deinit(st: &mut AppState, parent: &Path) {
    git(parent, &["submodule", "deinit", "-q", "-f", PATH]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(sub(st).state, SubmoduleState::Uninitialized);
}

fn commit_inside(st: &mut AppState, parent: &Path) {
    let dir = parent.join(PATH);
    std::fs::write(dir.join("a.txt"), "inside\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "inside"]);
    drive(st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(sub(st).state, SubmoduleState::OutOfSync);
}

pub(super) fn cleanup(dirs: &[&Path]) {
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
        [Effect::LoadRepo { kind: LoadKind::Open, path, .. }] => assert!(same_place(path, &want), "{}", path.display()),
        other => panic!("{other:?}"),
    }
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    finish(&mut st, fx);
    assert!(same_place(&st.snap.as_ref().unwrap().workdir, &want));

    // Opening it again from the parent selects its tab.
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
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
    drive(&mut st, msg(PATH, false));
    assert!(st.job.is_none());
    match &st.modal {
        Some(Modal::Confirm { then, title, body, button }) => {
            assert_eq!(title, "Update submodule");
            // The commit is on the submodule's main branch, so nothing is left behind.
            assert!(body.contains("Branches inside") && button == "Update", "{body}");
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
fn update_checks_out_recorded_commit_whatever_the_update_mode() {
    let (mut st, parent, lib) = opened("sm-update-mode");
    // Rebase would keep the commit made inside (it already contains the recorded one).
    git(&parent, &["config", "submodule.libs/lib.update", "rebase"]);
    commit_inside(&mut st, &parent);
    drive(&mut st, msg(PATH, true));
    let sm = sub(&st);
    assert_eq!(sm.state, SubmoduleState::InSync);
    assert_eq!(sm.checked_out, Some(sm.recorded));
    cleanup(&[&parent, &lib]);
}

#[test]
fn status_reload_refreshes_submodules() {
    let (mut st, parent, lib) = opened("sm-status-reload");
    // Already dirty, so the reload below stays a status reload rather than a full load.
    std::fs::write(parent.join("untracked.txt"), "x\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let dir = parent.join(PATH);
    std::fs::write(dir.join("a.txt"), "inside\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "inside"]);
    assert_eq!(sub(&st).state, SubmoduleState::InSync);
    let fx = update(&mut st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadStatus { submodules: true, .. }]), "{fx:?}");
    let loaded = fx.into_iter().filter_map(runner::execute).collect::<Vec<_>>();
    let fx: Vec<Effect> = loaded.into_iter().flat_map(|m| update(&mut st, m, Instant::now())).collect();
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "{fx:?}");
    assert_eq!(sub(&st).state, SubmoduleState::OutOfSync);
    cleanup(&[&parent, &lib]);
}

/// The commit `git submodule update` leaves: HEAD detached at the recorded commit, then a commit
/// made there. Returns its id.
fn commit_detached(parent: &Path, text: &str) -> Oid {
    let dir = parent.join(PATH);
    git(&dir, &["checkout", "-q", "--detach"]);
    std::fs::write(dir.join("a.txt"), text).unwrap();
    git(&dir, &["commit", "-q", "-am", text.trim()]);
    git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap()
}

#[test]
fn update_names_commits_it_would_leave_behind() {
    let (mut st, parent, lib) = opened("sm-left-behind");
    let first = commit_detached(&parent, "first\n");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(sub(&st).left_behind, vec![(first, "first".to_owned())]);
    // A second commit after the last full load: the snapshot doesn't know it yet.
    let second = commit_detached(&parent, "second\n");
    assert_eq!(sub(&st).left_behind.len(), 1);

    let fx = update(&mut st, msg(PATH, false), Instant::now());
    assert!(matches!(&fx[..], [Effect::Submodule(SubmoduleEffect::Check { path, .. })] if path == PATH), "{fx:?}");
    assert!(st.modal.is_none() && st.job.is_none());
    finish_checks(&mut st, fx);
    let left = sub(&st).left_behind;
    assert_eq!(left.iter().map(|c| c.0).collect::<Vec<_>>(), vec![second, first], "the row is read again");
    match &st.modal {
        Some(Modal::Confirm { body, button, then, .. }) => {
            let short = |o: Oid| o.to_string()[..7].to_owned();
            assert!(body.contains(&format!("\n    {} second", short(second))), "{body}");
            assert!(body.contains(&format!("\n    {} first", short(first))), "{body}");
            assert!(!body.contains("Branches inside"), "{body}");
            assert_eq!(button, "Update and leave 2 commits behind");
            assert!(matches!(&**then, Msg::Submodule(SubmoduleMsg::Update { confirmed: true, .. })));
        }
        _ => panic!("expected a confirmation"),
    }
    // Cancelling runs nothing; a branch inside then keeps the commits, and the warning goes.
    st.modal = None;
    git(&parent.join(PATH), &["branch", "keep"]);
    drive(&mut st, msg(PATH, false));
    match &st.modal {
        Some(Modal::Confirm { body, button, .. }) => {
            assert!(body.contains("Branches inside") && button == "Update", "{body}");
        }
        _ => panic!("expected a confirmation"),
    }
    cleanup(&[&parent, &lib]);
}

#[test]
fn checked_submodule_gone_or_unreadable_is_logged() {
    let (mut st, parent, lib) = opened("sm-checked-gone");
    let checked = |result| Msg::Submodule(SubmoduleMsg::Checked { path: PATH.into(), result });
    assert!(update(&mut st, checked(Ok(None)), Instant::now()).is_empty());
    assert!(last_log(&st).1.contains("no longer in this repository"));
    assert!(update(&mut st, checked(Err("boom".into())), Instant::now()).is_empty());
    assert_eq!(last_log(&st), (false, "Could not read submodule libs/lib: boom".to_owned()));
    assert!(st.modal.is_none());
    cleanup(&[&parent, &lib]);
}

/// Runs the `SubmoduleEffect::Check` effects in `fx` and handles their results.
fn finish_checks(st: &mut AppState, fx: Vec<Effect>) {
    for e in fx.into_iter().filter(|e| matches!(e, Effect::Submodule(SubmoduleEffect::Check { .. }))) {
        if let Some(m) = runner::execute(e) {
            drive(st, m);
        }
    }
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
        then_pr: None,
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

#[test]
fn submodule_update_reads_its_path_literally() {
    // `lib[1]` as a glob also matches `lib1`: only the submodule named exactly is updated.
    let lib = scratch_repo("sm-literal-lib");
    let parent = scratch_repo("sm-literal");
    let url = format!("../{}", lib.file_name().unwrap().to_string_lossy());
    // git checks for an existing entry with a glob too: the bracketed one goes in first.
    for p in ["lib[1]", "lib1"] {
        git(&parent, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, p]);
    }
    git(&parent, &["commit", "-q", "-m", "add libs"]);
    git(&parent, &["submodule", "deinit", "-q", "-f", "--all"]);
    let checked_out = |p: &str| std::fs::read_dir(parent.join(p)).map(|d| d.count() > 0).unwrap_or(false);
    assert!(!checked_out("lib1") && !checked_out("lib[1]"));
    let (ok, out) = trellis_core::gitcli::run(&parent, &update_args("lib[1]"));
    assert!(ok, "{out}");
    assert!(checked_out("lib[1]"));
    assert!(!checked_out("lib1"), "the glob reading would have updated lib1 too");
    cleanup(&[&parent, &lib]);
}
