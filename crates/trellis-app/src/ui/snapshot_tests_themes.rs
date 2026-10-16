//! Snapshots of the high-contrast and Sand themes, and of Preferences with a theme file
//! selected (its problems listed) or missing.

use super::snapshot_tests::{harness_palette, remove_fixture, settings, with_repo};
use super::snapshot_tests_diff::with_word_diff_in;
use super::theme::{self, color::Palette, custom};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use std::path::Path;
use std::time::Instant;
use trellis_core::settings::themes::parse;
use trellis_core::settings::{DiffLayout, ThemeCatalog, ThemeChoice, ThemeProblem};

fn render(name: &str, st: AppState, p: Palette) {
    let mut h = harness_palette(st, p);
    h.run();
    h.check_snapshot(name);
}

#[test]
fn main_high_contrast_dark() {
    let (st, dir) = with_repo(ThemeChoice::HighContrastDark, "theme-hc-dark");
    render("main_high_contrast_dark", st, theme::HIGH_CONTRAST_DARK);
    remove_fixture(&dir);
}

#[test]
fn main_sand() {
    let (st, dir) = with_repo(ThemeChoice::Sand, "theme-sand");
    render("main_sand", st, theme::SAND);
    remove_fixture(&dir);
}

#[test]
fn diff_high_contrast_light() {
    let (st, dir) = with_word_diff_in(DiffLayout::Inline, ThemeChoice::HighContrastLight, "theme-hc-light-diff");
    render("diff_high_contrast_light", st, theme::HIGH_CONTRAST_LIGHT);
    remove_fixture(&dir);
}

#[test]
fn prefs_ui_custom_theme() {
    let mut st = AppState::new(settings(ThemeChoice::Custom("ocean".into())), Vec::new(), Instant::now());
    let text = br##"{"name": "Ocean", "base": "dark", "colors": {"accent": "#ff8a3d", "selected": "#4a2a12", "hover": "selected"}}"##;
    let mut ocean = parse("ocean", Path::new("ocean.json"), text);
    ocean.problems =
        vec![ThemeProblem::UnknownColor("acent".into()), ThemeProblem::BadValue { key: "panel".into(), value: "\"#12345\"".into() }];
    let palette = custom::build(&ocean);
    st.themes.catalog = ThemeCatalog { files: vec![ocean], ..ThemeCatalog::default() };
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    render("prefs_ui_custom_theme", st, palette);
}

#[test]
fn prefs_ui_custom_missing() {
    let mut st = AppState::new(settings(ThemeChoice::Custom("gone".into())), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    render("prefs_ui_custom_missing", st, theme::DARK);
}
