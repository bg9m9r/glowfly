//! The first-run welcome in other looks: the Start step in Light and high contrast dark, the
//! Connect step in high contrast light, the cards stacked in a narrow window, and the footer
//! kept on screen in a short one.

use super::snapshot_tests::{harness_palette, harness_sized};
use super::snapshot_tests_onboarding::{ada, on, snapshot};
use crate::state::onboarding::WelcomeStep;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme;
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::identity::Identity;
use glowfly_core::settings::ThemeChoice;

#[test]
fn welcome_start_light() {
    snapshot("welcome_start_light", on(ThemeChoice::Light, WelcomeStep::Start, Some(ada())), false);
}

#[test]
fn welcome_high_contrast() {
    let mut h = harness_palette(on(ThemeChoice::HighContrastDark, WelcomeStep::Start, Some(ada())), theme::HIGH_CONTRAST_DARK);
    h.run();
    h.check_snapshot("welcome_start_high_contrast_dark");
    drop(h);
    let mut h = harness_palette(on(ThemeChoice::HighContrastLight, WelcomeStep::Connect, Some(ada())), theme::HIGH_CONTRAST_LIGHT);
    h.run();
    h.check_snapshot("welcome_connect_high_contrast_light");
}

#[test]
fn welcome_start_narrow() {
    let (w, hgt) = (600.0_f32, 640.0_f32);
    let mut h = harness_sized(on(ThemeChoice::Dark, WelcomeStep::Start, Some(ada())), true, egui::vec2(w, hgt));
    h.run();
    h.check_snapshot("welcome_start_narrow");
    let card = |label: &str| h.get_all_by_label(label).last().and_then(|n| n.bounding_box()).expect(label);
    let cards = ["Open a repository", "Clone a repository", "Start a new repository"].map(card);
    assert!(cards.iter().all(|c| (c.x0 - cards[0].x0).abs() < 0.5), "the cards share a left edge: {cards:?}");
    assert!(cards[0].y0 < cards[1].y0 && cards[1].y0 < cards[2].y0, "stacked: {cards:?}");
    let back = h.get_by_label("Back").bounding_box().unwrap();
    let done = h.get_by_label("Done").bounding_box().unwrap();
    for r in cards.iter().chain([&back, &done]) {
        assert!(r.x0 >= 0.0 && r.y0 >= 0.0 && r.x1 <= f64::from(w) && r.y1 <= f64::from(hgt), "inside the window: {r:?}");
    }
    let close = h.get_by_label("Close the welcome").bounding_box().unwrap();
    assert!(close.x1 <= 576.0, "the dialog keeps 24 px from the window's sides: {close:?}");
}

#[test]
fn welcome_short_window() {
    let (w, hgt) = (1400.0_f32, 480.0_f32);
    let mut h = harness_sized(on(ThemeChoice::Dark, WelcomeStep::Identity, Some(Identity::default())), true, egui::vec2(w, hgt));
    h.run();
    let cont = h.get_by_role_and_label(Role::Button, "Continue").bounding_box().unwrap();
    assert!(cont.y0 >= 0.0 && cont.y1 <= f64::from(hgt), "the footer stays on screen while the body scrolls: {cont:?}");
    let step = h.get_by_label("Step 1 of 4: You").bounding_box().unwrap();
    assert!(step.y0 >= 0.0 && step.y1 <= f64::from(hgt), "the header stays on screen: {step:?}");
}
