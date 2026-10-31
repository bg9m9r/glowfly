//! Headless tests of the release notes dialog: opening once after an upgrade (and not on a new
//! install's welcome), waiting for another dialog to close, and which releases are new.

use super::update;
use super::updates_tests::{app, release, send};
use crate::effect::Effect;
use crate::msg::{CheckedRelease, Msg, UpdateMsg as U};
use crate::state::{AppState, Modal};
use glowfly_core::app_update::{InstallKind, Version};
use std::time::Instant;

const NOTES: &str = "# Changelog

## [0.2.0](https://github.com/bg9m9r/glowfly/compare/v0.1.0...v0.2.0) (2026-10-20)

### Features

* **ui:** the new thing

## [0.1.5](https://github.com/bg9m9r/glowfly/compare/v0.1.0...v0.1.5) (2026-10-10)

### Bug Fixes

* a fix

## 0.1.0 (2026-10-01)

### Features

* first release
";

/// 0.2.0 starting with `seen` as the last version whose notes were shown.
fn upgraded(seen: Option<&str>) -> AppState {
    let mut st = app();
    st.settings.updates.notes_seen = seen.map(str::to_owned);
    st.updates.bundled_notes = NOTES;
    st
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::SaveSettings(_)))
}

fn dialog(st: &AppState) -> Option<&crate::state::release_notes::NotesDialog> {
    match &st.modal {
        Some(Modal::ReleaseNotes(d)) => Some(d),
        _ => None,
    }
}

#[test]
fn notes_open_once_after_an_upgrade() {
    let mut st = upgraded(None);
    let fx = send(&mut st, U::Startup, Instant::now());
    let d = dialog(&st).expect("the notes open");
    assert_eq!((d.current.as_str(), d.releases.len(), d.since.clone()), ("0.2.0", 3, None));
    assert_eq!(st.settings.updates.notes_seen.as_deref(), Some("0.2.0"));
    assert!(saves(&fx));
    // The next start says nothing.
    st.modal = None;
    let fx = send(&mut st, U::Startup, Instant::now());
    assert!(st.modal.is_none() && !saves(&fx));
}

#[test]
fn releases_since_the_last_seen_are_new() {
    let mut st = upgraded(Some("0.1.0"));
    send(&mut st, U::Startup, Instant::now());
    let d = dialog(&st).expect("open");
    assert_eq!(d.since, Version::parse("0.1.0"));
    let new: Vec<bool> = d.releases.iter().map(|r| d.is_new(r)).collect();
    assert_eq!(new, [true, true, false]);
}

#[test]
fn a_new_install_only_records_the_version() {
    let mut st = upgraded(None);
    st.settings.welcome_done = false;
    update(&mut st, Msg::Onboarding(crate::onboarding::OnboardingMsg::Startup), Instant::now());
    assert!(st.onboarding.welcome.is_some());
    let fx = send(&mut st, U::Startup, Instant::now());
    assert!(dialog(&st).is_none() && st.updates.notes_pending.is_none());
    assert_eq!(st.settings.updates.notes_seen.as_deref(), Some("0.2.0"));
    assert!(saves(&fx));
}

#[test]
fn notes_without_this_version_are_not_shown() {
    let mut st = upgraded(Some("0.1.0"));
    st.updates.current = Version::parse("0.2.1").unwrap();
    send(&mut st, U::Startup, Instant::now());
    assert!(dialog(&st).is_none());
    assert_eq!(st.settings.updates.notes_seen.as_deref(), Some("0.2.1"), "recorded anyway");
}

#[test]
fn notes_wait_for_an_open_dialog() {
    let mut st = upgraded(Some("0.1.0"));
    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::ClearLog) });
    let t = Instant::now();
    send(&mut st, U::Startup, t);
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })), "not replaced");
    assert!(st.updates.notes_pending.is_some());
    update(&mut st, Msg::Tick(t), t);
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })), "still waiting");
    update(&mut st, Msg::CloseModal, t);
    update(&mut st, Msg::Tick(t), t);
    let d = dialog(&st).expect("opened on the first free tick");
    assert_eq!(d.since, Version::parse("0.1.0"));
    assert!(st.updates.notes_pending.is_none());
}

#[test]
fn the_dialog_shows_the_offer_on_top() {
    let mut st = upgraded(Some("0.2.0"));
    let t = Instant::now();
    send(&mut st, U::ShowNotes, t);
    assert!(dialog(&st).is_some_and(|d| d.offer.is_none() && d.since.is_none()));
    let found = Ok(Some(CheckedRelease { release: release("v0.3.0", true), install: InstallKind::Installer }));
    send(&mut st, U::Checked { manual: true, result: found }, t);
    send(&mut st, U::ShowNotes, t);
    let d = dialog(&st).unwrap();
    assert_eq!(d.offer.as_ref().map(|o| o.version.to_string()).as_deref(), Some("0.3.0"));
    let notes = d.offered_notes.as_ref().unwrap();
    assert_eq!((notes.version.as_str(), notes.sections[0].title.as_str()), ("0.3.0", "Features"));
    update(&mut st, Msg::CloseModal, t);
    assert!(st.modal.is_none());
}
