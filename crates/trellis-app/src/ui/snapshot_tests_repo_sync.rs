//! The fetch or pull across repositories dialog: a run in progress (waiting, running and
//! finished rows, a failure and a skipped repository) and a finished one. The paths are labels
//! only, never read.

use super::snapshot_tests::{harness, settings};
use crate::msg::Msg;
use crate::state::repo_sync::{RepoSync, RowState, SyncKind, SyncRow, SyncScope};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo_sync::Outcome;
use trellis_core::settings::ThemeChoice;

/// A path that is absolute on every system (`C:/…` on Windows, `/…` elsewhere).
fn abs(rel: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { format!("C:/{rel}") } else { format!("/{rel}") })
}

fn row(name: &str, state: RowState) -> SyncRow {
    SyncRow { path: abs(&format!("src/{name}")), name: name.into(), state }
}

fn dialog(theme: ThemeChoice, kind: SyncKind, rows: Vec<SyncRow>) -> AppState {
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
        row("docs", RowState::Done(Outcome::Failed("Could not read from remote repository.".into()))),
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
        "Failed: Could not read from remote repository.",
        "Nothing to fetch: no remotes",
        "Skipped: not trusted. Open it to review its trust.",
        "Waiting",
        "Close (keeps running)",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label} is shown");
    }
    assert_eq!(h.query_all_by_label("Running…").count(), 2);
    h.check_snapshot("repo_sync_running");
    h.get_by_label("Close (keeps running)").click();
    h.step();
    h.step();
    assert!(matches!(&h.state().1[..], [Msg::CloseModal]));
}

#[test]
fn repo_sync_finished_light() {
    let rows = vec![
        row("api-server", RowState::Done(Outcome::Updated { changed: 2 })),
        row("billing", RowState::Done(Outcome::NoUpstream)),
        row("docs", RowState::Done(Outcome::Failed("Not possible to fast-forward, aborting.".into()))),
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
