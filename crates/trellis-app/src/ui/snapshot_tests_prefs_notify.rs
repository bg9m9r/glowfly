//! Preferences > UI Customization rows: Desktop notifications, whose checkbox only asks update
//! to change the setting (which saves it and sends the confirming notification), and Group
//! worktree tabs, bound to the setting directly.

use super::snapshot_tests::{harness, harness_sized, render, settings};
use crate::msg::{Msg, PrefsMsg};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use std::time::Instant;
use trellis_core::settings::ThemeChoice;

const LABEL: &str = "Show system notifications while Trellis is in the background";

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

#[test]
fn group_worktree_tabs_row() {
    let mut h = harness_sized(page(false), true, egui::vec2(1400.0, 1640.0));
    h.run();
    h.check_snapshot("prefs_ui_group_worktree_tabs");
    assert!(h.state().0.settings.group_worktree_tabs, "on by default");
    h.get_by_label("Automatically group worktree tabs together").click();
    h.run();
    assert!(!h.state().0.settings.group_worktree_tabs);
}
