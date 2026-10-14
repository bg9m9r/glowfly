//! Preferences > Git Flow snapshots: not set up yet, set up (with the Start buttons and the
//! Finish row), edited with overlapping prefixes, the Start dialog for a release, and the Finish
//! dialog for a release and for a feature blocked by uncommitted changes.

use super::snapshot_tests::{git, reload, remove_fixture, render, with_repo};
use crate::state::gitflow::{FinishForm, StartForm};
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use std::path::PathBuf;
use trellis_core::gitflow::FlowKind;
use trellis_core::repo::LoadOptions;
use trellis_core::settings::ThemeChoice;

/// Opens the Git Flow page with its form filled from the snapshot.
fn open_page(st: &mut AppState) {
    let mut prefs = PrefsState::new(PrefPage::RepoGitFlow);
    prefs.gitflow.fill(st.snap.as_ref().unwrap());
    st.prefs = Some(prefs);
}

/// The fixture with a develop branch and the gitflow config (version tag prefix v).
fn initialized(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-05T10:00:00+00:00";
    git(&dir, &["branch", "develop"], d);
    for (key, value) in [
        ("gitflow.branch.master", "main"),
        ("gitflow.branch.develop", "develop"),
        ("gitflow.prefix.feature", "feature/"),
        ("gitflow.prefix.release", "release/"),
        ("gitflow.prefix.hotfix", "hotfix/"),
        ("gitflow.prefix.versiontag", "v"),
    ] {
        git(&dir, &["config", key, value], d);
    }
    reload(&mut st, &dir, &LoadOptions::all(500));
    (st, dir)
}

#[test]
fn prefs_gitflow_setup() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "gitflow-setup");
    open_page(&mut st);
    render("prefs_gitflow_setup", st, false);
    remove_fixture(&dir);
}

#[test]
fn prefs_gitflow() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-set-up");
    open_page(&mut st);
    render("prefs_gitflow", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_gitflow_edited() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-edited");
    open_page(&mut st);
    let form = &mut st.prefs.as_mut().unwrap().gitflow.form;
    form.feature = "feat/".into();
    form.release = "feat/".into();
    render("prefs_gitflow_edited", st, true);
    remove_fixture(&dir);
}

#[test]
fn gitflow_start_release() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-start");
    st.modal = Some(Modal::GitFlowStart(StartForm { kind: FlowKind::Release, name: "0.2.0".into() }));
    render("gitflow_start_release", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_gitflow_finish() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-finish-row");
    git(&dir, &["branch", "release/0.2.0"], "2026-01-05T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    open_page(&mut st);
    render("prefs_gitflow_finish", st, true);
    remove_fixture(&dir);
}

#[test]
fn gitflow_finish_release() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-finish-release");
    let d = "2026-01-05T10:00:00+00:00";
    git(&dir, &["checkout", "--", "README.md"], d);
    git(&dir, &["branch", "release/0.2.0"], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.modal = Some(Modal::GitFlowFinish(FinishForm {
        kind: FlowKind::Release,
        branch: "release/0.2.0".into(),
        tag: "v0.2.0".into(),
        message: "Release 0.2.0".into(),
        keep: false,
        focus_tag: true,
    }));
    render("gitflow_finish_release", st, true);
    remove_fixture(&dir);
}

#[test]
fn gitflow_finish_feature_blocked() {
    let (mut st, dir) = initialized(ThemeChoice::Light, "gitflow-finish-blocked");
    git(&dir, &["branch", "feature/search", "search"], "2026-01-05T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.modal = Some(Modal::GitFlowFinish(FinishForm {
        kind: FlowKind::Feature,
        branch: "feature/search".into(),
        tag: String::new(),
        message: String::new(),
        keep: false,
        focus_tag: false,
    }));
    render("gitflow_finish_feature_blocked", st, false);
    remove_fixture(&dir);
}
