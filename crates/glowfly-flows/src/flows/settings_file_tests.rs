//! Headless tests for a settings file Glowfly couldn't read: a damaged one moved aside is
//! reported once and saving goes on; one that couldn't be read at all holds every save back
//! until Try again reads it, and the settings read then replace the defaults.

use crate::effect::{Effect, OsEffect, PrefsEffect};
use crate::flows::state;
use crate::msg::{Msg, PrefsMsg};
use crate::state::AppState;
use crate::update::update;
use glowfly_core::settings::{LoadProblem, Settings};
use std::path::PathBuf;
use std::time::Instant;

/// A damaged settings file that was moved aside: saving is safe.
pub(crate) fn damaged() -> LoadProblem {
    LoadProblem { backup: Some(PathBuf::from("settings.json.bak")), reset: None, detail: "expected value".into(), locked: None }
}

/// The text `damaged` is logged and toasted with.
pub(crate) const DAMAGED_TEXT: &str =
    "Settings could not be read and were reset to defaults (expected value). The original file was kept as settings.json.bak.";

fn file() -> PathBuf {
    std::env::temp_dir().join("glowfly-settings-locked").join("settings.json")
}

fn unreadable(detail: &str) -> LoadProblem {
    LoadProblem { backup: None, reset: None, detail: detail.into(), locked: Some(file()) }
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

/// Flips a setting that saves at once when changed (the toolbar's labels).
fn toggle_labels(st: &mut AppState) -> Vec<Effect> {
    let on = !st.settings.toolbar_labels;
    run(st, Msg::Prefs(PrefsMsg::SetToolbarLabels(on)))
}

#[test]
fn a_damaged_file_moved_aside_is_reported_once_and_saving_goes_on() {
    let mut st = state();
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::SettingsUnreadable(damaged())));
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.log.iter().any(|l| !l.ok && l.text == DAMAGED_TEXT));
    assert!(!st.view.show_log);
    assert_eq!(st.toasts.len(), 1);
    assert!(crate::flows::logs_tabs_tests::failure_with_details(&st), "its toast offers Details");
    assert!(st.settings_file.writable());
    assert_eq!(saves(&toggle_labels(&mut st)), 1);
}

#[test]
fn an_unreadable_file_holds_every_save_back_until_a_try_reads_it() {
    let mut st = state();
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::SettingsUnreadable(unreadable("access is denied"))));
    assert!(fx.is_empty(), "{fx:?}");
    assert!(!st.settings_file.writable());
    assert!(st.log.iter().any(|l| !l.ok && l.text.contains("access is denied")), "logged");
    assert!(st.toasts.is_empty(), "the banner says it, not a toast");
    let before = st.settings_file.saved.clone();

    // Changes apply but aren't written over the real file.
    let labels = st.settings.toolbar_labels;
    let fx = toggle_labels(&mut st);
    assert_eq!(saves(&fx), 0, "{fx:?}");
    assert_ne!(st.settings.toolbar_labels, labels, "the change still applies");
    assert_eq!(st.settings_file.saved, before, "not recorded as saved");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(crate::state::PrefPage::General)));
    st.settings.auto_fetch_minutes = 42;
    assert_eq!(saves(&run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited))), 0, "Preferences doesn't save either");
    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));

    // Open settings folder shows the file's folder.
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenSettingsFolder));
    let folder = file().parent().unwrap().to_owned();
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::OpenFolder(d))] if *d == folder), "{fx:?}");

    // Try again reads once at a time; a try that fails again keeps saving held back.
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::RetrySettingsRead));
    assert!(matches!(&fx[..], [Effect::Prefs(PrefsEffect::ReadSettings)]), "{fx:?}");
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::RetrySettingsRead)).is_empty(), "already reading");
    let still = Box::new((Settings::default(), Some(unreadable("in use by another process"))));
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsReread(still)));
    let u = st.settings_file.unreadable.as_ref().expect("still unreadable");
    assert_eq!((u.detail.as_str(), u.reading), ("in use by another process", false));
    assert_eq!(st.toasts.len(), 1, "told the try failed");
    assert_eq!(saves(&toggle_labels(&mut st)), 0);

    // A try that reads it: its settings replace the defaults, and saving goes on.
    run(&mut st, Msg::Prefs(PrefsMsg::RetrySettingsRead));
    let real = Settings { auto_fetch_minutes: 17, toolbar_labels: true, ..Settings::default() };
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsReread(Box::new((real.clone(), None)))));
    assert!(st.settings_file.writable());
    assert_eq!(st.settings.auto_fetch_minutes, 17, "the file's settings, not the edits made meanwhile");
    assert_eq!(st.settings_file.saved, serde_json::to_string(&real).unwrap());
    assert!(st.toasts.iter().any(|t| t.ok), "told it worked");
    let fx = toggle_labels(&mut st);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.auto_fetch_minutes == 17 && !s.toolbar_labels), "{fx:?}");
}

#[test]
fn a_late_try_result_after_the_file_was_read_is_ignored() {
    let mut st = state();
    let real = Settings { auto_fetch_minutes: 17, ..Settings::default() };
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsReread(Box::new((real, None)))));
    assert_ne!(st.settings.auto_fetch_minutes, 17, "nothing was waiting for it");
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::RetrySettingsRead)).is_empty());
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::OpenSettingsFolder)).is_empty());
}
