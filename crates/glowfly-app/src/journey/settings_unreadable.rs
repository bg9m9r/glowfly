//! A settings file the startup load couldn't read: the app comes up with the banner, none of
//! its startup steps writes the defaults over the file, and Try again reads it again.

use super::find::{Pick, Region};
use super::fixture::Fixture;
use super::harness::Journey;
use glowfly_core::settings::LoadProblem;

#[test]
fn an_unreadable_settings_file_is_never_written_over() {
    let fx = Fixture::new("settings-unreadable");
    let file = fx.config().join("settings.json");
    let problem = LoadProblem { backup: None, reset: None, detail: "access is denied".into(), locked: Some(file.clone()) };
    let mut j = Journey::start_unreadable(fx, problem);
    j.see("Glowfly couldn't read its settings file", Region::Anywhere);
    assert!(j.logged("access is denied"), "the activity log says why");
    assert!(!file.exists(), "no startup step saved the defaults");
    j.click("Try again", Region::Anywhere, Pick::Only);
    assert!(j.dropped().contains(&"read-settings"), "the sandbox drops the read of the real file: {:?}", j.dropped());
    assert!(!file.exists());
}
