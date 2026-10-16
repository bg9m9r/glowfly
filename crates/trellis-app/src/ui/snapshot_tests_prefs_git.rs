//! Preferences > Git: the PATH git, the other gits found (an old one warned about, a refused one
//! disabled), a git picked with Browse… earlier, and the version in use. Rows only ask update to
//! check a git; they never change the setting themselves.

use super::snapshot_tests::{harness_sized, settings};
use crate::git_env::GitEnvMsg;
use crate::msg::Msg;
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::git_program::{Candidate, Refusal};
use trellis_core::git_version::GitInfo;
use trellis_core::native_path::shown;
use trellis_core::settings::ThemeChoice;

/// A path for this OS's spelling; only shown, never touched.
fn at(windows: &str, unix: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { windows } else { unix })
}

fn path_git() -> PathBuf {
    at("C:/Program Files/Git/cmd/git.exe", "/opt/homebrew/bin/git")
}

fn old_git() -> PathBuf {
    at("C:/Users/ada/scoop/apps/git/current/cmd/git.exe", "/usr/bin/git")
}

fn broken_git() -> PathBuf {
    at("C:/msys64/usr/bin/git.exe", "/usr/local/bin/git")
}

fn page(theme: ThemeChoice, git_program: Option<PathBuf>) -> AppState {
    let mut s = settings(theme);
    s.git_program = git_program.as_ref().map(|p| p.display().to_string());
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Git));
    st
}

fn found() -> Vec<Candidate> {
    vec![
        Candidate { path: path_git(), first_on_path: true, check: Ok((2, 45, 1)) },
        Candidate { path: old_git(), first_on_path: false, check: Ok((2, 34, 1)) },
        Candidate { path: broken_git(), first_on_path: false, check: Err(Refusal::TimedOut.text()) },
    ]
}

fn listed(theme: ThemeChoice) -> AppState {
    let mut st = page(theme, None);
    st.git.info = Some(GitInfo { path: Some(path_git()), version: Some((2, 45, 1)), chosen: false });
    st.git.page.candidates = Some(found());
    st.git.page.refusal = Some((shown(&at("C:/Downloads/git-gui.exe", "/Applications/Utilities/gitk")), Refusal::Launcher.text()));
    st
}

const SIZE: egui::Vec2 = egui::vec2(1400.0, 900.0);

#[test]
fn preferences_git() {
    let mut h = harness_sized(listed(ThemeChoice::Dark), true, SIZE);
    h.run();
    h.check_snapshot("prefs_git");
}

#[test]
fn preferences_git_custom_light() {
    let custom = at("D:/PortableGit/cmd/git.exe", "/opt/git/2.47/bin/git");
    let mut st = page(ThemeChoice::Light, Some(custom.clone()));
    st.git.info = Some(GitInfo { path: Some(custom), version: Some((2, 47, 0)), chosen: true });
    st.git.page.candidates = Some(Vec::new());
    let mut h = harness_sized(st, false, SIZE);
    h.run();
    h.check_snapshot("prefs_git_custom_light");
}

#[test]
fn a_git_row_only_asks() {
    let mut h = harness_sized(listed(ThemeChoice::Dark), true, SIZE);
    h.run();
    h.get_by_label(&shown(&old_git())).click();
    h.run();
    let (st, msgs) = h.state();
    assert!(msgs.iter().any(|m| matches!(m, Msg::GitEnv(GitEnvMsg::Choose(Some(p))) if *p == old_git())));
    assert_eq!(st.settings.git_program, None, "update owns the change");

    let mut h = harness_sized(listed(ThemeChoice::Dark), true, SIZE);
    h.run();
    h.get_by_label(&shown(&broken_git())).click();
    h.run();
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::GitEnv(GitEnvMsg::Choose(_)))), "a refused git can't be chosen");
    h.get_by_label("Look again").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::GitEnv(GitEnvMsg::Rescan))));
}
