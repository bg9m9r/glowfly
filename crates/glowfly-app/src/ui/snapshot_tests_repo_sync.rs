//! The fetch or pull across repositories dialog: a run in progress (waiting, running and
//! finished rows, a failure and a skipped repository, with Stop, Retry and Open), one stopping,
//! a failure's whole error on hover, and a finished run. The paths are labels only, never read.

use super::snapshot_tests::{harness, settings};
use crate::msg::{Msg, RepoMsg, ReposMsg};
use crate::state::repo_sync::{RepoSync, RowState, SyncKind, SyncRow, SyncScope};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo_sync::Outcome;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::Instant;

/// A path that is absolute on every system (`C:/…` on Windows, `/…` elsewhere).
fn abs(rel: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { format!("C:/{rel}") } else { format!("/{rel}") })
}

pub(super) fn row(name: &str, state: RowState) -> SyncRow {
    SyncRow { path: abs(&format!("src/{name}")), name: name.into(), state }
}

pub(super) fn dialog(theme: ThemeChoice, kind: SyncKind, rows: Vec<SyncRow>) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.repo_sync = Some(RepoSync { generation: 1, kind, scope: SyncScope::Listed, rows });
    st.modal = Some(Modal::SyncRepos);
    st
}

#[test]
fn repo_sync_running() {
    let rows = vec![
        row("api-server", RowState::Done(Outcome::Updated { changed: 3 })),
        row("billing", RowState::Done(Outcome::UpToDate)),
        row("docs", RowState::Done(failed("Could not read from remote repository.", ""))),
        row("notes", RowState::Done(Outcome::NoRemotes)),
        row("scripts", RowState::Done(Outcome::Untrusted)),
        row("web-app", RowState::Running),
        row("a-repository-with-a-very-long-folder-name", RowState::Running),
        row("website", RowState::Waiting),
    ];
    let mut h = harness(dialog(ThemeChoice::Dark, SyncKind::Fetch, rows), true);
    h.step();
    h.step();
    h.step();
    for label in [
        "Fetch all listed repositories",
        "Fetching: 5 of 8 done",
        "Fetched: 3 branches or tags moved",
        "Up to date",
        "Nothing to fetch: no remotes",
        "Skipped: not trusted. Open it to review its trust.",
        "Waiting",
        "Close (keeps running)",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    assert!(h.query_by_label_contains("Failed: Couldn't read from the remote repository.").is_some(), "in plain words");
    assert_eq!(h.query_all_by_label("Running…").count(), 2);
    // Finished rows (five) have Open; only the failed one has Retry.
    assert_eq!(h.query_all_by_label("Open").count(), 5);
    assert_eq!(h.query_all_by_label("Retry").count(), 1);
    assert!(!h.get_by_label("Stop").is_disabled());
    h.check_snapshot("repo_sync_running");
    h.get_by_label("Stop").click();
    steps(&mut h);
    assert!(matches!(&h.state().1[..], [Msg::Repos(ReposMsg::StopSync)]));
    h.state_mut().1.clear();
    h.get_by_label("Retry").click();
    steps(&mut h);
    assert!(matches!(&h.state().1[..], [Msg::Repos(ReposMsg::RetrySync { generation: 1, index: 2 })]));
    h.state_mut().1.clear();
    h.get_all_by_label("Open").nth(2).unwrap().click();
    steps(&mut h);
    match &h.state().1[..] {
        [Msg::CloseModal, Msg::Repo(RepoMsg::Open(p))] => assert_eq!(*p, abs("src/docs")),
        other => panic!("{}", other.len()),
    }
    h.state_mut().1.clear();
    h.get_by_label("Close (keeps running)").click();
    steps(&mut h);
    assert!(matches!(&h.state().1[..], [Msg::CloseModal]));
}

fn failed(reason: &str, detail: &str) -> Outcome {
    Outcome::Failed { reason: reason.into(), detail: detail.into() }
}

/// A few frames: the dialog's spinners keep repainting, so `run` would not settle.
fn steps(h: &mut Harness<'_, (AppState, Vec<Msg>)>) {
    for _ in 0..3 {
        h.step();
    }
}

#[test]
fn repo_sync_stopping() {
    let rows = vec![
        row("api-server", RowState::Done(Outcome::UpToDate)),
        row("billing", RowState::Running),
        row("docs", RowState::Running),
        row("notes", RowState::Stopped),
        row("website", RowState::Stopped),
    ];
    let mut h = harness(dialog(ThemeChoice::Dark, SyncKind::Fetch, rows), true);
    steps(&mut h);
    assert!(h.query_by_label("Stopping: 2 still running").is_some());
    assert!(h.get_by_label("Stop").is_disabled(), "nothing waits");
    assert_eq!(h.query_all_by_label("Stopped").count(), 2);
    assert_eq!(h.query_all_by_label("Retry").count(), 2, "stopped rows can run again");
    assert!(h.query_by_label("Close (keeps running)").is_some());
    h.check_snapshot("repo_sync_stopping");
}

#[test]
fn repo_sync_error_tip() {
    let detail = "Cloning into 'docs'...\nfatal: unable to access 'https://***@example.com/docs.git/': Could not resolve host: example.com\nfatal: Could not read from remote repository.";
    let rows = vec![
        row("api-server", RowState::Done(Outcome::UpToDate)),
        row("docs", RowState::Done(failed("Could not read from remote repository.", detail))),
    ];
    let mut h = harness(dialog(ThemeChoice::Dark, SyncKind::Fetch, rows), true);
    h.run();
    h.get_by_label_contains("Failed: Couldn't reach example.com.").hover();
    for _ in 0..4 {
        h.step();
    }
    h.run_ok();
    assert!(h.query_by_label_contains("Could not resolve host: example.com").is_some(), "git's whole output shows");
    assert!(h.query_all_by_label_contains("Couldn't reach example.com.").count() >= 2, "the sentence leads the tip");
    h.check_snapshot("repo_sync_error_tip");
}

#[test]
fn repo_sync_finished_light() {
    let rows = vec![
        row("api-server", RowState::Done(Outcome::Updated { changed: 2 })),
        row("billing", RowState::Done(Outcome::NoUpstream)),
        row("docs", RowState::Done(failed("Not possible to fast-forward, aborting.", ""))),
    ];
    let mut h = harness(dialog(ThemeChoice::Light, SyncKind::Pull, rows), false);
    h.run();
    for label in [
        "Pull all listed repositories",
        "Pull of 3 repositories: 1 updated, 1 with nothing to pull, 1 failed.",
        "Pulled",
        "Nothing to pull: the branch has no upstream",
        "Close",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    h.check_snapshot("repo_sync_finished_light");
}

#[test]
fn repo_sync_stopping_is_translated() {
    use glowfly_core::i18n::Lang;
    let rows = vec![row("billing", RowState::Running), row("docs", RowState::Running), row("notes", RowState::Stopped)];
    let mut s = settings(ThemeChoice::Dark);
    s.language = "de".into();
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.repo_sync = Some(RepoSync { generation: 1, kind: SyncKind::Fetch, scope: SyncScope::Listed, rows });
    st.modal = Some(Modal::SyncRepos);
    let mut h = harness(st, true);
    steps(&mut h);
    let de = Lang::De;
    assert!(h.query_by_label(&de.trn("repo_sync.stopping", 2, &[])).is_some(), "the header is in German");
    assert!(h.query_by_label(de.tr("repo_sync.row.stopped")).is_some(), "the row state is in German");
    assert!(h.query_by_label("Stopped").is_none());
    assert!(h.query_by_label_contains("still running").is_none());
}
