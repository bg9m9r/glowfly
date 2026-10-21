//! Headless tests for Preferences > Diff viewer: the settings save once, change only how the open
//! diff is painted (no reload), and closing Preferences keeps the diff's top row.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::{AppState, PrefPage};
use glowfly_core::intraline;
use glowfly_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A tab-indented one-word edit, with its changed words marked as the runner would.
fn tabbed_diff() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(2), new: Some(2), text: text.into() };
    let hunks = vec![Hunk {
        header: "@@ -1,3 +1,3 @@".into(),
        lines: vec![line(' ', "fn f() {"), line('-', "\tlet x = 1;"), line('+', "\tlet x = 2;"), line(' ', "}")],
    }];
    let mut d = FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None };
    d.intraline = intraline::intraline(&d);
    d
}

/// A state with `tabbed_diff` open and the settings as saved.
fn with_diff() -> AppState {
    let mut st = state();
    st.diff = Some((DiffTarget::Unstaged("f.rs".into()), tabbed_diff().into()));
    // Bring the saved copy in line with the settings, so only the edits below count.
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    st
}

fn only_saves(fx: &[Effect]) -> bool {
    !fx.is_empty() && fx.iter().all(|e| matches!(e, Effect::SaveSettings(_)))
}

#[test]
fn editor_settings_save_once() {
    let mut st = with_diff();
    st.settings.diff_font_size = 16.0;
    st.settings.tab_size = 8;
    st.settings.diff_line_numbers = false;
    st.settings.diff_word_highlights = false;
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    let [Effect::SaveSettings(s)] = fx.as_slice() else { panic!("expected one save, got {fx:?}") };
    assert_eq!((s.diff_font_size, s.tab_size, s.diff_line_numbers, s.diff_word_highlights), (16.0, 8, false, false));
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty(), "nothing changed since the save");
}

#[test]
fn turning_word_highlights_off_keeps_the_ranges() {
    let mut st = with_diff();
    let before = st.diff.as_ref().unwrap().1.intraline.clone();
    assert!(!before.line(0, 1).is_empty(), "the -/+ pair is marked");
    st.settings.diff_word_highlights = false;
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    assert!(only_saves(&fx), "{fx:?}");
    assert_eq!(st.diff.as_ref().unwrap().1.intraline, before);
}

#[test]
fn changing_tab_size_does_not_reload() {
    let mut st = with_diff();
    st.settings.tab_size = 2;
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    assert!(only_saves(&fx), "{fx:?}");
    assert_eq!(st.diff.as_ref().unwrap().1, tabbed_diff().into(), "raw tabs kept; only the painting changes");
}

#[test]
fn open_diff_viewer_page() {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::DiffViewer)));
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::DiffViewer));
    assert_eq!(PrefPage::DiffViewer.title(glowfly_core::i18n::Lang::En), "Diff viewer");
}

#[test]
fn closing_prefs_keeps_the_diff_top_row() {
    let mut st = with_diff();
    st.view.diff_top_row = 7;
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::DiffViewer)));
    st.settings.diff_font_size = 16.0;
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    assert_eq!(st.view.diff_scroll_to_row, Some(7), "the same row stays at the top at the new row height");

    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::DiffViewer)));
    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    assert_eq!(st.view.diff_scroll_to_row, None, "no diff open");
}
