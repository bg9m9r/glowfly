//! Preferences rows: General's Desktop notifications, whose checkbox only asks update to change
//! the setting (which saves it and sends the confirming notification), and Confirm pushes and
//! pulls; Appearance's Group worktree tabs, bound to the setting directly, and the Windows system
//! title bar switch, shown only where Windows' frame is offered.

use super::snapshot_tests::{harness, harness_sized, render, settings};
use crate::msg::{Msg, PrefsMsg};
use crate::state::window_chrome::Chrome;
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

const LABEL: &str = "Show system notifications while Glowfly is in the background";

fn page(on: bool) -> AppState {
    on_page(on, PrefPage::Ui)
}

fn on_page(on: bool, page: PrefPage) -> AppState {
    let mut s = settings(ThemeChoice::Dark);
    s.desktop_notifications = on;
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(page));
    st
}

#[test]
fn desktop_notifications_checkbox_only_asks() {
    let mut h = harness(on_page(false, PrefPage::General), true);
    h.run();
    h.get_by_label(LABEL).click();
    h.run();
    let (st, msgs) = h.state();
    assert!(msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::SetDesktopNotifications(true)))));
    assert!(!st.settings.desktop_notifications, "update owns the change");
}

#[test]
fn confirm_pushes_and_pulls_is_on_and_turns_off() {
    let mut h = harness(on_page(false, PrefPage::General), true);
    h.run();
    assert!(h.state().0.settings.confirm_push_pull, "on by default");
    h.get_by_label("Confirm pushes and pulls").click();
    h.run();
    assert!(!h.state().0.settings.confirm_push_pull);
}

#[test]
fn preferences_ui_desktop_on() {
    render("prefs_ui_desktop_on", on_page(true, PrefPage::General), true);
}

const TITLE_BAR: &str = "Use the Windows title bar";

#[test]
fn system_title_bar_row_is_offered_on_windows_only() {
    let size = egui::vec2(1400.0, 1720.0);
    let mut h = harness_sized(page(false), true, size);
    h.run();
    assert!(h.query_by_label(TITLE_BAR).is_none(), "not offered off Windows");
    drop(h);
    let mut st = page(false);
    st.view.chrome = Chrome::Windows;
    st.view.system_title_bar_offered = true;
    let mut h = harness_sized(st, true, size);
    h.run();
    h.check_snapshot("prefs_ui_system_title_bar");
    assert!(!h.state().0.settings.system_title_bar, "off by default");
    h.get_by_label(TITLE_BAR).click();
    h.run();
    assert!(h.state().0.settings.system_title_bar);
    assert_eq!(h.state().0.view.chrome, Chrome::Windows, "the frame changes at the next start");
    drop(h);
    // After that restart: the system frame, with its menu bar row, and the switch to go back.
    let mut st = page(false);
    st.settings.system_title_bar = true;
    st.view.system_title_bar_offered = true;
    let mut h = harness_sized(st, true, size);
    h.run();
    assert!(h.query_by_label("Show the menu bar").is_some());
    h.get_by_label(TITLE_BAR).click();
    h.run();
    assert!(!h.state().0.settings.system_title_bar);
}

#[test]
fn group_worktree_tabs_row() {
    let mut h = harness_sized(page(false), true, egui::vec2(1400.0, 1640.0));
    h.run();
    h.check_snapshot("prefs_ui_group_worktree_tabs");
    assert!(h.state().0.settings.group_worktree_tabs, "on by default");
    h.get_by_label("Keep a repository's worktree tabs side by side").click();
    h.run();
    assert!(!h.state().0.settings.group_worktree_tabs);
}
