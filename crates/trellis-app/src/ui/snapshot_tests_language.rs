//! Language snapshots: Preferences > General in German (long labels must fit their column), the
//! Language menu open, External Tools in Spanish with its warnings, and the main window's commit
//! box and status bar in German; and switching the language from the menu takes effect on the
//! next frame.

use crate::state::prefs::{Autocrlf, GitConfigValues};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, harness_sized, remove_fixture, settings, with_repo};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::time::Instant;
use trellis_core::git_version::GitInfo;
use trellis_core::settings::{ExternalTools, ThemeChoice};

/// Preferences open on `page`, with the settings in `language`.
fn prefs(theme: ThemeChoice, page: PrefPage, language: &str) -> AppState {
    let mut s = settings(theme);
    s.language = language.into();
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(page));
    st
}

/// The General page as `preferences_general` draws it: git config read, a git found.
fn general_with_git(language: &str) -> AppState {
    let mut st = prefs(ThemeChoice::Dark, PrefPage::General, language);
    if let Some(p) = st.prefs.as_mut() {
        p.git = Some(GitConfigValues { autocrlf: Autocrlf::Input, ..Default::default() });
    }
    let program = if cfg!(windows) { "C:/Program Files/Git/cmd/git.exe" } else { "/opt/homebrew/bin/git" };
    st.git.info = Some(GitInfo { path: Some(program.into()), version: Some((2, 45, 1)), chosen: false });
    st
}

#[test]
fn prefs_general_german() {
    let mut h = harness_sized(general_with_git("de"), true, egui::vec2(1400.0, 1700.0));
    h.run();
    h.check_snapshot("prefs_general_german");
}

#[test]
fn prefs_general_language_menu_light() {
    let st = prefs(ThemeChoice::Light, PrefPage::General, "system");
    let mut h = harness(st, false);
    h.run();
    h.get_by_role(Role::ComboBox).click();
    h.run();
    for entry in ["Deutsch", "English", "Español"] {
        assert!(h.query_by_label(entry).is_some(), "{entry} is offered");
    }
    h.check_snapshot("prefs_general_language_menu_light");
}

#[test]
fn language_applies_without_restart() {
    let mut h = harness(prefs(ThemeChoice::Dark, PrefPage::General, "system"), true);
    h.run();
    assert!(h.query_all_by_label("General").count() > 0);
    h.get_by_role(Role::ComboBox).click();
    h.run();
    h.get_by_label("Deutsch").click();
    h.run();
    assert_eq!(h.state().0.settings.language, "de", "the menu edits the setting in place");
    assert!(h.query_all_by_label("Allgemein").count() > 0, "the navigation is German on the next frame");
    assert_eq!(h.query_all_by_label("General").count(), 0);
}

#[test]
fn prefs_tools_spanish_light() {
    let mut st = prefs(ThemeChoice::Light, PrefPage::Tools, "es");
    st.settings.external_tools = ExternalTools {
        editor: "code".into(),
        editor_file_args: "--goto \"{path}:{line}:{col}".into(),
        diff_tool: "meld".into(),
        diff_tool_args: "--left {local}".into(),
        ..ExternalTools::default()
    };
    let mut h = harness_sized(st, false, egui::vec2(1400.0, 1560.0));
    h.run();
    h.check_snapshot("prefs_tools_spanish_light");
}

#[test]
fn main_window_german() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "language-de");
    st.settings.language = "de".into();
    st.settings.show_toolbar = false;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("main_window_german");
    drop(h);
    remove_fixture(&dir);
}
