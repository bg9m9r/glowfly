//! Preferences > Appearance rows: Desktop notifications, whose checkbox only asks update
//! to change the setting (which saves it and sends the confirming notification), Group
//! worktree tabs, bound to the setting directly, and the Windows system title bar switch, shown
//! only where Windows' frame is offered.

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
    let mut s = settings(ThemeChoice::Dark);
    s.desktop_notifications = on;
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    st
}

#[test]
fn desktop_notifications_checkbox_only_asks() {
    let mut h = harness(page(false), true);
    h.run();
    h.get_by_label(LABEL).click();
    h.run();
    let (st, msgs) = h.state();
    assert!(msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::SetDesktopNotifications(true)))));
    assert!(!st.settings.desktop_notifications, "update owns the change");
}

#[test]
fn preferences_ui_desktop_on() {
    render("prefs_ui_desktop_on", page(true), true);
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
