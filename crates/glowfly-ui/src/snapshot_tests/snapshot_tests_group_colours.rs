//! Group colours' default: the group menu's Reset all group colors (enabled while a group's
//! colour differs from its default, off with its reason once none does), in both themes, and
//! Preferences > UI Customization's New group color combo, open, with its swatches. The listed
//! paths are labels only, never opened.

use super::snapshot_tests::{harness_sized, settings};
use super::snapshot_tests_repo_list::{H, abs, api, last, web, welcome_state};
use super::snapshot_tests_select::click;
use crate::msg::{GroupMsg, Msg};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{GroupColourDefault, RepoColour, RepoGroup, ThemeChoice};
use std::time::Instant;

const RESET: &str = "Reset all group colors";

/// Favorites web-app, api-server (both in Work, Rose) and docs; Home, empty, in `home`.
fn welcome(theme: ThemeChoice, home: Option<RepoColour>) -> AppState {
    let mut st = welcome_state(theme);
    st.settings.favorite_repos = vec![web(), api(), abs("docs")];
    let mut work = RepoGroup::new("Work", Some(RepoColour::Rose));
    work.repos = vec![web(), api()];
    st.settings.repo_groups = vec![work, RepoGroup::new("Home", home)];
    st
}

/// The start page with Work's group menu open.
fn group_menu(st: AppState, dark: bool) -> H {
    let mut h = harness_sized(st, dark, vec2(1400.0, 1040.0));
    h.run();
    let more = h.get_all_by_label("Group actions").next().unwrap().bounding_box().unwrap();
    click(&mut h, pos2(((more.x0 + more.x1) / 2.0) as f32, ((more.y0 + more.y1) / 2.0) as f32), PointerButton::Primary);
    h.run();
    h
}

#[test]
fn welcome_repo_group_menu_light() {
    let mut h = group_menu(welcome(ThemeChoice::Light, None), false);
    assert!(!h.get_by_label(RESET).is_disabled(), "Home has no colour, its default is Teal");
    h.check_snapshot("welcome_repo_group_menu_light");
    h.get_by_label(RESET).click();
    h.run();
    assert!(matches!(last(&h), Msg::Groups(GroupMsg::ResetColours)));
}

#[test]
fn welcome_repo_group_menu_reset_blocked() {
    let mut h = group_menu(welcome(ThemeChoice::Dark, Some(RepoColour::Teal)), true);
    assert!(h.get_by_label(RESET).is_disabled(), "Rose and Teal are the defaults");
    h.check_snapshot("welcome_repo_group_menu_reset_blocked");
    drop(h);
    // Another default makes the same colours worth resetting.
    let mut st = welcome(ThemeChoice::Dark, Some(RepoColour::Teal));
    st.settings.new_group_colour = GroupColourDefault::None;
    let h = group_menu(st, true);
    assert!(!h.get_by_label(RESET).is_disabled());
}

/// Preferences > UI Customization with New group color at `chosen`, tall enough for the
/// Interface rows and the combo's open list below it, the combo open.
fn prefs(theme: ThemeChoice, dark: bool, chosen: GroupColourDefault) -> H {
    let mut s = settings(theme);
    s.new_group_colour = chosen;
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    let mut h = harness_sized(st, dark, vec2(1400.0, 2000.0));
    h.run();
    h.get_by_label("New group color").click();
    h.run();
    h
}

#[test]
fn prefs_ui_group_colour() {
    let mut h = prefs(ThemeChoice::Dark, true, GroupColourDefault::Next);
    for label in ["Next unused color", "None", "Rose", "Plum"] {
        assert!(h.query_by_label(label).is_some(), "the combo offers {label}");
    }
    h.check_snapshot("prefs_ui_group_colour");
    h.get_by_label("Teal").click();
    h.run();
    assert_eq!(h.state().0.settings.new_group_colour, GroupColourDefault::Hue(RepoColour::Teal));
}

#[test]
fn prefs_ui_group_colour_light() {
    // A hue chosen: its swatch on the closed combo too.
    let mut h = prefs(ThemeChoice::Light, false, GroupColourDefault::Hue(RepoColour::Teal));
    h.check_snapshot("prefs_ui_group_colour_light");
}
