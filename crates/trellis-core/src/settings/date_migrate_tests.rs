//! Old settings files: the built-in date patterns older builds saved load blank, so they follow
//! the date locale; a pattern the user typed loads unchanged.

use super::Settings;
use super::tests::scratch_settings;

const OLD: &str = r#""date_format": "MM/DD/YYYY", "date_word_format": "MMMM D, YYYY",
    "date_time_format": "MM/DD/YYYY @ h:mm A", "date_verbose_format": "MMMM D, YYYY h:mm A""#;

fn blank(date: &str, word: &str, time: &str, verbose: &str) -> bool {
    [date, word, time, verbose].iter().all(|p| p.is_empty())
}

#[test]
fn old_built_in_patterns_load_blank() {
    let (dir, file) = scratch_settings("old-dates", format!("{{{OLD}}}").as_bytes());
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(problem, None);
    assert!(blank(&s.date_format, &s.date_word_format, &s.date_time_format, &s.date_verbose_format));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn typed_patterns_load_unchanged() {
    let text = br#"{"date_time_format": "YYYY-MM-DD HH:mm", "date_format": "MM/DD/YYYY"}"#;
    let (dir, file) = scratch_settings("typed-dates", text);
    let (s, _) = Settings::load_from(&file);
    assert_eq!(s.date_time_format, "YYYY-MM-DD HH:mm");
    assert_eq!(s.date_format, "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saved_profiles_forget_old_patterns_too() {
    let text = format!(r#"{{"profiles": [{{"id": 2, "name": "Work", "prefs": {{{OLD}}}}}]}}"#);
    let (dir, file) = scratch_settings("profile-dates", text.as_bytes());
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(problem, None);
    let p = s.profiles.iter().find_map(|p| p.prefs.as_ref()).expect("saved prefs");
    assert!(blank(&p.date_format, &p.date_word_format, &p.date_time_format, &p.date_verbose_format));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn legacy_presets_still_migrate() {
    let (dir, file) = scratch_settings("iso-dates", br#"{"date_format": "Iso", "relative_dates": true}"#);
    let (s, _) = Settings::load_from(&file);
    assert!(!s.relative_dates);
    assert_eq!(s.date_format, "");
    let _ = std::fs::remove_dir_all(&dir);
}
