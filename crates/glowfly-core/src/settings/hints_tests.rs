//! The tips' settings: on by default, missing keys load the defaults, and ids a newer version
//! wrote (unknown here) survive a load and save.

use super::Settings;
use super::{json, scratch_settings};
use std::collections::BTreeSet;

#[test]
fn a_file_without_the_keys_loads_the_defaults() {
    let (dir, file) = scratch_settings("hints-old", br#"{"auto_fetch_minutes": 5}"#);
    let (s, problem) = Settings::load_from(&file);
    assert!(problem.is_none());
    assert!(s.hints, "tips are on unless turned off");
    assert!(s.hints_seen.is_empty() && s.hints_ignored.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_ids_survive_a_round_trip() {
    let text = br#"{"hints": false, "hints_seen": ["undo", "from-a-newer-version"], "hints_ignored": ["later-tip"]}"#;
    let (dir, file) = scratch_settings("hints-unknown", text);
    let (s, problem) = Settings::load_from(&file);
    assert!(problem.is_none());
    assert!(!s.hints);
    let seen: BTreeSet<String> = ["undo", "from-a-newer-version"].map(String::from).into();
    assert_eq!(s.hints_seen, seen);
    s.save_to(&file).unwrap();
    let (back, _) = Settings::load_from(&file);
    assert_eq!(json(&back), json(&s));
    assert!(back.hints_seen.contains("from-a-newer-version") && back.hints_ignored.contains("later-tip"));
    let _ = std::fs::remove_dir_all(&dir);
}
