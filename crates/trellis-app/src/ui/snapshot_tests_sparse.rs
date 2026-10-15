//! Preferences > Sparse Checkout snapshots: off, on in folders mode, patterns mode with both
//! warnings, an edited form with a refused path, and a job running.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::state::{AppState, PrefPage, PrefsState, RunningJob};
use crate::ui::snapshot_check::CheckSnapshot;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;
use trellis_core::sparse_ops::{SparseMode, SparseStatus};

/// The fixture repository with the Sparse Checkout page showing `status`, its form filled.
fn sparse_page(theme: ThemeChoice, name: &str, status: SparseStatus) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let mut prefs = PrefsState::new(PrefPage::RepoSparse);
    prefs.sparse.generation = 1;
    prefs.sparse.status = Some(status);
    prefs.sparse.fill();
    st.prefs = Some(prefs);
    (st, dir)
}

fn on(mode: SparseMode, paths: &[&str]) -> SparseStatus {
    SparseStatus {
        enabled: true,
        mode,
        paths: paths.iter().map(|p| (*p).to_owned()).collect(),
        tracked: 340,
        skipped: 328,
        ..Default::default()
    }
}

#[test]
fn prefs_sparse_off() {
    let (st, dir) = sparse_page(ThemeChoice::Dark, "sparse-off", SparseStatus { tracked: 340, ..Default::default() });
    render("prefs_sparse_off", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_sparse_cone() {
    let (st, dir) = sparse_page(ThemeChoice::Light, "sparse-cone", on(SparseMode::Cone, &["src", "docs/guide"]));
    render("prefs_sparse_cone", st, false);
    remove_fixture(&dir);
}

#[test]
fn prefs_sparse_patterns() {
    let status = SparseStatus { fell_back: true, sparse_index: true, ..on(SparseMode::Patterns, &["/*", "!/assets/", "/assets/icons/"]) };
    let (st, dir) = sparse_page(ThemeChoice::Dark, "sparse-patterns", status);
    render("prefs_sparse_patterns", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_sparse_edited_error() {
    let (mut st, dir) = sparse_page(ThemeChoice::Dark, "sparse-edited", on(SparseMode::Cone, &["src"]));
    let page = &mut st.prefs.as_mut().unwrap().sparse;
    page.paths = "src\nassets/*.png".into();
    page.paths_error = Some(trellis_core::sparse_ops::PathProblem::Pattern("assets/*.png".into()).message());
    render("prefs_sparse_edited_error", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_sparse_busy() {
    let (mut st, dir) = sparse_page(ThemeChoice::Dark, "sparse-busy", on(SparseMode::Cone, &["src", "docs/guide"]));
    st.job = Some(RunningJob {
        label: crate::state::sparse::SAVE_LABEL.into(),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: false,
    });
    // The status bar's job spinner keeps repainting; a fixed number of frames keeps the image
    // reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("prefs_sparse_busy");
    remove_fixture(&dir);
}
