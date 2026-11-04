//! The status bar's What's new chip after an upgrade (dark and light, beside an update's chip,
//! cut in a narrow window) and Preferences > General's choice of what an upgrade shows. Click
//! tests check the chip opens the notes and the choice is kept.

use super::{app, prefs, sent, status_bar, the_offer};
use crate::snapshot_check::CheckSnapshot;
use crate::snapshot_tests::{NOW, harness, harness_sized};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{NewsChip, ThemeChoice, WhatsNew};
use glowfly_model::msg::UpdateMsg;
use glowfly_model::state::AppState;
use glowfly_model::state::updates::UpdateStatus;

const CHIP: &str = "What's new in 0.2.0";

/// 0.2.0 started after 0.1.0, its chip waiting a day more.
fn with_chip(mut st: AppState) -> AppState {
    st.updates.shown = "0.2.0";
    st.settings.updates.notes_seen = Some("0.2.0".into());
    st.settings.updates.news_chip =
        Some(NewsChip { version: "0.2.0".into(), since: Some("0.1.0".into()), until: Some(NOW + 24 * 60 * 60) });
    st
}

#[test]
fn status_whats_new() {
    let mut h = harness(with_chip(status_bar(UpdateStatus::Idle)), true);
    h.run();
    h.check_snapshot("status_whats_new_dark");
    h.get_by_label(CHIP).click();
    h.run();
    assert!(sent(&h, &UpdateMsg::ShowNotes));
}

#[test]
fn status_whats_new_light() {
    let mut h = harness(with_chip(app(ThemeChoice::Light)), false);
    h.run();
    h.check_snapshot("status_whats_new_light");
}

#[test]
fn status_whats_new_tip() {
    let mut h = harness(with_chip(status_bar(UpdateStatus::Idle)), true);
    h.run();
    h.get_by_label(CHIP).hover();
    h.run();
    h.run();
    assert!(h.query_by_label_contains("Show the release notes for this version").is_some());
}

/// A newer release found after the upgrade: both chips, the update's nearer the version.
#[test]
fn status_whats_new_beside_an_update() {
    let mut h = harness(with_chip(status_bar(UpdateStatus::Available(Box::new(the_offer())))), true);
    h.run();
    h.check_snapshot("status_whats_new_beside_update");
    let update = h.get_by_label("Update available (0.3.0)").bounding_box().unwrap();
    let news = h.get_by_label(CHIP).bounding_box().unwrap();
    assert!(news.x1 <= update.x0, "{news:?} {update:?}");
}

#[test]
fn the_whats_new_chip_hides_with_another_choice_or_when_its_time_is_up() {
    let mut st = with_chip(status_bar(UpdateStatus::Idle));
    st.settings.updates.whats_new = WhatsNew::Off;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(CHIP).is_none(), "Off");
    let mut st = with_chip(status_bar(UpdateStatus::Idle));
    st.settings.updates.news_chip.as_mut().unwrap().until = Some(NOW);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(CHIP).is_none(), "run out");
}

#[test]
fn prefs_updates_whats_new() {
    let mut h = harness_sized(prefs(UpdateStatus::UpToDate), true, egui::vec2(1240.0, 900.0));
    h.run();
    h.check_snapshot("prefs_updates_whats_new");
    assert!(h.query_by_label("Show “What's new” in the status bar").is_some());
    h.get_by_label("Open the release notes").click();
    h.run();
    assert_eq!(h.state().0.settings.updates.whats_new, WhatsNew::Dialog);
    h.get_by_label("Don't show anything").click();
    h.run();
    assert_eq!(h.state().0.settings.updates.whats_new, WhatsNew::Off);
}
