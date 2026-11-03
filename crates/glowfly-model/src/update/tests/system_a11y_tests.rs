//! Headless tests for the system's accessibility settings (`AppMsg::SystemA11y`): each value in
//! effect (motion, contrast, the zoom) with Preferences following the system and not, and that
//! the system never changes a choice made in Preferences.

use super::tests::state;
use super::update;
use crate::msg::{AppMsg, Msg};
use crate::state::AppState;
use crate::state::system_a11y::SystemA11y;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

fn system(st: &mut AppState, s: SystemA11y) {
    update(st, Msg::App(AppMsg::SystemA11y(s)), Instant::now());
}

fn asks(reduce_motion: bool, high_contrast: bool, text_scale: f32) -> SystemA11y {
    SystemA11y { reduce_motion, high_contrast, text_scale, ..SystemA11y::default() }
}

#[test]
fn the_message_records_what_the_system_asks_for() {
    let mut st = state();
    assert_eq!(st.view.system_a11y, SystemA11y::default());
    system(&mut st, asks(true, true, 1.5));
    assert_eq!(st.view.system_a11y, asks(true, true, 1.5));
    system(&mut st, SystemA11y::default());
    assert_eq!(st.view.system_a11y, SystemA11y::default(), "turned off again in the system");
}

#[test]
fn motion_is_reduced_by_the_choice_or_the_followed_system() {
    let mut st = state();
    assert!(!st.motion_reduced());
    system(&mut st, asks(true, false, 1.0));
    assert!(st.motion_reduced(), "followed by default");
    st.settings.a11y.follow_system_motion = false;
    assert!(!st.motion_reduced(), "not followed");
    st.settings.reduce_motion = true;
    system(&mut st, SystemA11y::default());
    assert!(st.motion_reduced(), "the choice stays whatever the system says");
    assert!(st.settings.reduce_motion);
}

#[test]
fn system_contrast_shows_without_rewriting_the_chosen_theme() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Mist;
    assert!(!st.system_contrast());
    system(&mut st, asks(false, true, 1.0));
    assert!(st.system_contrast());
    assert_eq!((st.settings.theme.clone(), st.settings.theme_before_contrast.clone()), (ThemeChoice::Mist, None), "untouched");
    st.settings.a11y.follow_system_contrast = false;
    assert!(!st.system_contrast(), "not followed");
    st.settings.a11y.follow_system_contrast = true;
    // A high-contrast theme chosen in Preferences already shows: nothing to put in its place.
    st.settings.theme = ThemeChoice::HighContrastLight;
    assert!(!st.system_contrast());
    system(&mut st, SystemA11y::default());
    assert_eq!(st.settings.theme, ThemeChoice::HighContrastLight, "the choice stays when the system turns it off");
}

#[test]
fn the_system_text_size_scales_the_interface_size() {
    let mut st = state();
    st.settings.zoom_percent = 100;
    system(&mut st, asks(false, false, 1.25));
    assert_eq!(st.zoom_factor(), 1.25);
    // Snapped to a zoom step (110% at 125% is 138%, nearer 150% than 125%), and never past the
    // largest.
    st.settings.zoom_percent = 110;
    assert_eq!(st.zoom_factor(), 1.5);
    system(&mut st, asks(false, false, 2.25));
    assert_eq!(st.zoom_factor(), 2.0);
    st.settings.a11y.follow_system_text_size = false;
    assert_eq!(st.zoom_factor(), 1.1, "not followed: the interface size alone");
    assert_eq!(st.settings.zoom_percent, 110, "never rewritten");
    st.settings.a11y.follow_system_text_size = true;
    for odd in [f32::NAN, 0.0, -1.0] {
        system(&mut st, asks(false, false, odd));
        assert_eq!(st.zoom_factor(), 1.1, "{odd} is ignored");
    }
}
