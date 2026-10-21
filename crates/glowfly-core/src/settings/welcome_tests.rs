//! The first-run welcome flag: new installs see the welcome, existing files never do.

use super::Settings;
use super::tests::{json, scratch_settings};

#[test]
fn welcome_flag_defaults() {
    assert!(!Settings::default().welcome_done, "a new install shows the welcome");
    let (dir, file) = scratch_settings("welcome-old", br#"{"auto_fetch_minutes": 5}"#);
    let (s, problem) = Settings::load_from(&file);
    assert!(s.welcome_done && problem.is_none(), "a file from before the welcome counts as seen");
    std::fs::write(&file, br#"{"welcome_done": false}"#).unwrap();
    assert!(!Settings::load_from(&file).0.welcome_done, "a first run quit midway keeps false");
    for done in [false, true] {
        let s = Settings { welcome_done: done, ..Settings::default() };
        s.save_to(&file).unwrap();
        let (back, problem) = Settings::load_from(&file);
        assert_eq!((json(&back), problem), (json(&s), None));
    }
    let _ = std::fs::remove_dir_all(&dir);
    let missing = dir.join("settings.json");
    assert!(!Settings::load_from(&missing).0.welcome_done, "no file at all is a new install");
}

#[test]
fn welcome_flag_survives_a_lenient_load() {
    let (dir, file) = scratch_settings("welcome-lenient", br#"{"theme": "Sepia", "default_branch": "trunk"}"#);
    let (s, problem) = Settings::load_from(&file);
    assert_eq!(problem.unwrap().reset, Some(vec!["theme".to_owned()]));
    assert!(s.welcome_done && s.default_branch == "trunk");
    let _ = std::fs::remove_dir_all(&dir);
    let (dir, file) = scratch_settings("welcome-array", b"[1, 2]");
    let (s, problem) = Settings::load_from(&file);
    assert!(problem.unwrap().reset.is_none(), "a full reset");
    assert!(s.welcome_done, "the file existed, so its owner isn't new");
    let _ = std::fs::remove_dir_all(&dir);
}
