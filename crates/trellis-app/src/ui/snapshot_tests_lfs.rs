//! Preferences > Git LFS snapshots: set up with patterns, not set up yet (with a hook in the
//! way), git-lfs missing in a repository that uses LFS, and a job running.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::state::{AppState, PrefPage, PrefsState, RunningJob};
use crate::ui::snapshot_check::CheckSnapshot;
use std::path::PathBuf;
use trellis_core::lfs::{LfsPattern, LfsStatus};
use trellis_core::settings::ThemeChoice;

fn pattern(raw: &str, lockable: bool) -> LfsPattern {
    LfsPattern { raw: raw.into(), display: trellis_core::lfs::unescape_pattern(raw), lockable }
}

fn patterns() -> Vec<LfsPattern> {
    vec![pattern("*.psd", false), pattern("*.blend", true), pattern("my[[:space:]]dir/*.bin", false)]
}

/// The fixture repository with an origin remote and the LFS page showing `status`.
fn lfs_page(theme: ThemeChoice, name: &str, status: LfsStatus) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![])];
    let mut prefs = PrefsState::new(PrefPage::RepoLfs);
    prefs.lfs.generation = 1;
    prefs.lfs.status = Some(status);
    st.prefs = Some(prefs);
    (st, dir)
}

fn enabled() -> LfsStatus {
    LfsStatus { version: Some("3.7.1".into()), initialized: true, patterns: patterns(), has_gitattributes: true, foreign_hooks: vec![] }
}

#[test]
fn prefs_lfs() {
    let (st, dir) = lfs_page(ThemeChoice::Dark, "lfs-enabled", enabled());
    render("prefs_lfs", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_lfs_setup() {
    let status = LfsStatus {
        version: Some("3.7.1".into()),
        initialized: false,
        patterns: vec![pattern("*.psd", false)],
        has_gitattributes: true,
        foreign_hooks: vec!["pre-push".into()],
    };
    let (st, dir) = lfs_page(ThemeChoice::Light, "lfs-setup", status);
    render("prefs_lfs_setup", st, false);
    remove_fixture(&dir);
}

#[test]
fn prefs_lfs_missing() {
    let status = LfsStatus {
        version: None,
        initialized: false,
        patterns: vec![pattern("*.psd", false), pattern("*.blend", true)],
        has_gitattributes: true,
        foreign_hooks: vec![],
    };
    let (st, dir) = lfs_page(ThemeChoice::Dark, "lfs-missing", status);
    render("prefs_lfs_missing", st, true);
    remove_fixture(&dir);
}

#[test]
fn prefs_lfs_busy() {
    let (mut st, dir) = lfs_page(ThemeChoice::Dark, "lfs-busy", enabled());
    st.job = Some(RunningJob {
        label: "LFS pull".into(),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: false,
    });
    // The status bar's job spinner keeps repainting; a fixed number of frames (egui's clock
    // advances by a fixed step) keeps the image reproducible. The page itself draws no spinner.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("prefs_lfs_busy");
    remove_fixture(&dir);
}
