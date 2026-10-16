//! The update preferences: on by default, and kept across a save and load.

use super::tests::{json, scratch_settings};
use super::{Settings, UpdatePrefs};

#[test]
fn update_prefs_default_on() {
    let (dir, file) = scratch_settings("updates-old", br#"{"auto_fetch_minutes": 5}"#);
    let (s, problem) = Settings::load_from(&file);
    assert!(problem.is_none());
    assert_eq!(s.updates, UpdatePrefs { check: true, last_checked: None, notes_seen: None });
    assert!(s.welcome_done, "an older file counts as an upgrade for the release notes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn update_prefs_round_trip() {
    let (dir, file) = scratch_settings("updates-trip", b"{}");
    let s = Settings {
        updates: UpdatePrefs { check: false, last_checked: Some(1_769_947_200), notes_seen: Some("0.2.0".into()) },
        ..Settings::default()
    };
    s.save_to(&file).unwrap();
    let (back, problem) = Settings::load_from(&file);
    assert_eq!((json(&back), problem), (json(&s), None));
    assert_eq!(back.updates.notes_seen.as_deref(), Some("0.2.0"));
    // A partial object keeps the other defaults.
    std::fs::write(&file, br#"{"updates": {"last_checked": 5}}"#).unwrap();
    let (s, _) = Settings::load_from(&file);
    assert_eq!(s.updates, UpdatePrefs { check: true, last_checked: Some(5), notes_seen: None });
    let _ = std::fs::remove_dir_all(&dir);
}
