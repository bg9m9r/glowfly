//! Preferences > Accessibility: its goldens (Graphite, Porcelain, German and Spanish in a 760 px
//! window, and the system asking for all three with high contrast showing), the Tab walk through every control in visual order, every control's name and
//! description, the checks (Announcements among them) and the High contrast check's message, Show all shortcuts, the
//! rows it shares with their own pages, and the keyboard focus holding a toast.

use crate::msg::{Msg, NotifyMsg, PrefsMsg};
use crate::state::system_a11y::SystemA11y;
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_fixtures::{harness_palette_sized, settings};
use crate::ui::theme::{self, color::Palette};
use eframe::egui::{Key, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{Settings, ThemeChoice};
use std::time::Instant;

/// Tall enough for the whole page, the Timing section included, without scrolling.
const TALL: f32 = 1320.0;
/// The same for German and Spanish in a 760 px window, where rows stack.
const DE_ES_TALL: f32 = 1640.0;

fn page(s: Settings, p: Palette, size: Vec2) -> Harness<'static, (AppState, Vec<Msg>)> {
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Accessibility));
    let mut h = harness_palette_sized(st, p, size);
    h.run();
    h
}

fn english() -> Harness<'static, (AppState, Vec<Msg>)> {
    page(settings(ThemeChoice::Graphite), theme::GRAPHITE, vec2(1400.0, TALL))
}

#[test]
fn prefs_accessibility() {
    english().check_snapshot("prefs_accessibility");
}

#[test]
fn prefs_accessibility_light() {
    page(settings(ThemeChoice::Porcelain), theme::PORCELAIN, vec2(1400.0, TALL)).check_snapshot("prefs_accessibility_light");
}

/// The system asks for less motion, high contrast and larger text, all followed: the page as the
/// shell shows it then, in High contrast dark in place of Graphite (the theme chosen stays).
#[test]
fn prefs_accessibility_system_on() {
    let mut st = AppState::new(settings(ThemeChoice::Graphite), Vec::new(), Instant::now());
    st.view.system_a11y = SystemA11y { reduce_motion: true, high_contrast: true, text_scale: 1.25, ..SystemA11y::default() };
    assert!(st.system_contrast() && st.motion_reduced());
    st.prefs = Some(PrefsState::new(PrefPage::Accessibility));
    let mut h = harness_palette_sized(st, theme::HIGH_CONTRAST_DARK, vec2(1400.0, TALL));
    h.run();
    h.check_snapshot("prefs_accessibility_system_on");
}

#[test]
fn prefs_accessibility_de() {
    let s = Settings { language: "de".into(), ..settings(ThemeChoice::Graphite) };
    page(s, theme::GRAPHITE, vec2(760.0, DE_ES_TALL)).check_snapshot("prefs_accessibility_de");
}

#[test]
fn prefs_accessibility_es() {
    let s = Settings { language: "es".into(), ..settings(ThemeChoice::Graphite) };
    page(s, theme::GRAPHITE, vec2(760.0, DE_ES_TALL)).check_snapshot("prefs_accessibility_es");
}

/// Every control on the page, in visual order, by its accessible name.
const CONTROLS: [&str; 16] = [
    "Interface size",
    "Interface size, Also follow the system's text size",
    "Diff text size",
    "Terminal text size",
    "High contrast, Use a high-contrast theme",
    "High contrast, Also when the system uses high contrast",
    "Colors",
    "Reduce motion, Turn off fades and slides",
    "Reduce motion, Also when the system asks for less motion",
    "Blinking cursor, Blink the text cursor",
    "Focus indicator",
    "Change shortcuts…",
    "Show all shortcuts",
    "Announcements, Announce when background work finishes or fails",
    "Notifications stay",
    "Tooltip delay",
];

#[test]
fn tab_reaches_every_control_in_visual_order() {
    let mut h = english();
    h.get_by_label(CONTROLS[0]).focus();
    h.run();
    assert!(h.get_by_label(CONTROLS[0]).is_focused());
    for name in &CONTROLS[1..] {
        h.press_key(Key::Tab);
        h.run();
        assert!(h.get_by_label(name).is_focused(), "Tab moves on to {name}");
    }
    // The last control lets the focus go on: no trap.
    h.press_key(Key::Tab);
    h.run();
    assert!(!h.get_by_label(CONTROLS[CONTROLS.len() - 1]).is_focused());
}

#[test]
fn every_control_is_named_and_described() {
    let h = english();
    // The zoom keys in the help are the keymap's, shown as on this OS.
    let keys = &h.state().0.keymap;
    let zoom = (keys.hint(crate::keymap::BindingId::ZoomIn).unwrap(), keys.hint(crate::keymap::BindingId::ZoomOut).unwrap());
    let zoom_help = format!("Scales all text and controls. {} and {} also change it.", zoom.0, zoom.1);
    let described = [
        (CONTROLS[0], zoom_help.as_str()),
        (CONTROLS[1], zoom_help.as_str()),
        (CONTROLS[4], "Switches to High contrast dark or light; turning it off brings back your theme."),
        (CONTROLS[5], "Switches to High contrast dark or light; turning it off brings back your theme."),
        (CONTROLS[6], "Branches, diffs and file status use colors that stay apart with color blindness."),
        (CONTROLS[9], "The cursor in text fields. The terminal's never blinks."),
        (CONTROLS[10], "Strong draws a thicker ring with an outline that shows on every color."),
        (CONTROLS[12], "Every command can be given a key."),
        (CONTROLS[13], "Read by screen readers such as NVDA, Narrator and VoiceOver."),
    ];
    for name in CONTROLS {
        assert!(h.query_by_label(name).is_some(), "{name} is named");
    }
    for (name, text) in described {
        assert_eq!(h.get_by_label(name).description().as_deref(), Some(text), "{name}");
    }
}

#[test]
fn checks_edit_in_place_and_high_contrast_asks() {
    let mut h = english();
    h.get_by_label(CONTROLS[7]).focus();
    h.run();
    h.press_key(Key::Space);
    h.run();
    assert!(h.state().0.settings.reduce_motion, "Space toggles a check");
    h.get_by_label(CONTROLS[9]).click();
    h.run();
    assert!(!h.state().0.settings.a11y.cursor_blink);
    h.get_by_label(CONTROLS[13]).click();
    h.run();
    assert!(!h.state().0.settings.a11y.announce_jobs, "Announcements, on by default, now off");
    h.get_by_label(CONTROLS[4]).click();
    h.run();
    let asked = h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::SetHighContrast { on: true, dark: true })));
    assert!(asked, "update switches the theme");
    assert_eq!(h.state().0.settings.theme, ThemeChoice::Graphite, "the view doesn't");
}

#[test]
fn the_follow_checks_edit_in_place() {
    let mut h = english();
    for name in [CONTROLS[1], CONTROLS[5], CONTROLS[8]] {
        h.get_by_label(name).click();
        h.run();
    }
    let a = &h.state().0.settings.a11y;
    assert!(!a.follow_system_text_size && !a.follow_system_contrast && !a.follow_system_motion, "all on by default, now off");
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::SetHighContrast { .. }))), "the theme isn't asked for");
}

#[test]
fn no_text_size_check_where_the_system_has_no_text_size() {
    let mut st = AppState::new(settings(ThemeChoice::Graphite), Vec::new(), Instant::now());
    st.view.system_a11y.text_size_followable = false;
    st.prefs = Some(PrefsState::new(PrefPage::Accessibility));
    let mut h = harness_palette_sized(st, theme::GRAPHITE, vec2(1400.0, TALL));
    h.run();
    assert!(h.query_by_label(CONTROLS[0]).is_some());
    assert!(h.query_by_label(CONTROLS[1]).is_none(), "macOS has no text size to follow");
    assert!(h.state().0.settings.a11y.follow_system_text_size, "the setting stays as it is");
}

#[test]
fn the_shortcut_buttons_go_to_keyboard_and_the_sheet() {
    let mut h = english();
    h.get_by_label(CONTROLS[11]).click();
    h.run();
    h.get_by_label(CONTROLS[12]).click();
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::Navigate(PrefPage::Keyboard)))));
    assert!(msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::ShowShortcutSheet))));
}

#[test]
fn closing_the_sheet_gives_the_focus_back_to_show_all_shortcuts() {
    let mut h = english();
    h.get_by_label(CONTROLS[12]).click();
    h.run();
    h.state_mut().0.modal = Some(crate::state::Modal::Shortcuts { filter: String::new() });
    h.run();
    h.state_mut().0.modal = None;
    h.run();
    h.run();
    assert!(h.get_by_label(CONTROLS[12]).is_focused());
}

#[test]
fn the_page_edits_no_mirrored_setting_itself() {
    // One home: the shared rows edit zoom and the text sizes; this page only calls them.
    let src = include_str!("accessibility.rs");
    for owned in ["&mut s.zoom_percent", "diff_font_size", ".font_size"] {
        assert!(!src.contains(owned), "{owned} belongs to its own page's row");
    }
}

#[test]
fn a_mirrored_row_edits_the_same_setting_as_its_page() {
    let mut h = english();
    h.get_by_label(CONTROLS[2]).focus();
    h.run();
    h.press_key(Key::ArrowUp);
    h.run();
    let s = &h.state().0.settings;
    assert!(s.diff_font_size > Settings::default().diff_font_size, "{}", s.diff_font_size);
}

#[test]
fn keyboard_focus_on_a_toast_holds_it() {
    let mut st = AppState::new(settings(ThemeChoice::Graphite), Vec::new(), Instant::now());
    st.toast(true, "Copied", Instant::now());
    let id = st.toasts[0].id;
    let mut h = harness_palette_sized(st, theme::GRAPHITE, vec2(900.0, 600.0));
    h.run();
    h.get_by_label("Dismiss").focus();
    h.run();
    h.run();
    let held = h.state().1.iter().any(|m| matches!(m, Msg::Notify(NotifyMsg::HoldToast(Some(t))) if *t == id));
    assert!(held, "the focus in a toast holds it as the pointer does");
}
