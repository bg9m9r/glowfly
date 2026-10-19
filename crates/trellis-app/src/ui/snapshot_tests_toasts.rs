//! Toasts: a plain one stacked with a behind alert whose Pull button sends `Msg::ToastAction`,
//! in the top and bottom corners, all one width and 12 px inside the chrome (under the toolbar,
//! or the tabs when it is hidden).

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::{AppState, ToastAction};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, containers::panel::PanelState};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::Path;
use std::time::Instant;
use trellis_core::settings::{Corner, ThemeChoice};

type H = Harness<'static, (AppState, Vec<Msg>)>;

const PLAIN: &str = "Fetch succeeded";
const ALERT: &str = "main is 2 commits behind origin/main";

/// Adds a plain toast and a behind alert for `dir`; returns the alert's id.
fn add_toasts(st: &mut AppState, dir: &Path) -> u64 {
    let now = Instant::now();
    st.toast(true, PLAIN, now);
    let action = ToastAction { label: "Pull".into(), msg: Box::new(Msg::git("Pull", &["pull", "--ff"])), workdir: Some(dir.to_path_buf()) };
    st.toast_action(true, ALERT, action, now)
}

/// The top and left edges of the node labelled `label`.
fn top_left(h: &H, label: &str) -> (f64, f64) {
    let b = h.get_by_label(label).bounding_box().expect("bounds");
    (b.y0, b.x0)
}

/// The two toasts' texts start at one x (one frame width in every corner).
fn lined_up(h: &H) {
    assert_eq!(top_left(h, PLAIN).1, top_left(h, ALERT).1, "the toasts line up");
}

#[test]
fn toast_behind_alert() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "toast-behind");
    st.settings.notification_location = Corner::TopRight;
    let id = add_toasts(&mut st, &dir);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("toast_behind_alert");
    lined_up(&h);
    let toolbar = h.get_by_label("Pull the current branch").bounding_box().expect("the toolbar's Pull").y1;
    assert!(top_left(&h, PLAIN).0 >= toolbar + 12.0, "under the toolbar");
    // The toast's button (the toolbar's is "Pull the current branch").
    h.get_by_label("Pull").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ToastAction(i) if *i == id)), "Pull sends the toast's action");
    remove_fixture(&dir);
}

#[test]
fn toast_behind_alert_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "toast-behind-light");
    st.settings.notification_location = Corner::TopRight;
    add_toasts(&mut st, &dir);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("toast_behind_alert_light");
    lined_up(&h);
    remove_fixture(&dir);
}

#[test]
fn toast_stacked_bottom_left() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "toast-bottom-left");
    st.settings.notification_location = Corner::BottomLeft;
    add_toasts(&mut st, &dir);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("toast_stacked_bottom_left");
    lined_up(&h);
    let status = PanelState::load(&h.ctx, egui::Id::new("status")).expect("the status bar").rect.top();
    let alert = h.get_by_label(ALERT).bounding_box().unwrap();
    assert!(alert.y1 < f64::from(status) - 12.0, "above the status bar");
    remove_fixture(&dir);
}

#[test]
fn toast_top_left_toolbar_hidden() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "toast-top-left");
    st.settings.notification_location = Corner::TopLeft;
    st.settings.show_toolbar = false;
    add_toasts(&mut st, &dir);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("toast_top_left_toolbar_hidden");
    lined_up(&h);
    let tabs = PanelState::load(&h.ctx, egui::Id::new("tabs")).expect("the tab strip").rect.bottom();
    let top = top_left(&h, PLAIN).0;
    assert!(top >= f64::from(tabs) + 12.0, "under the tabs: {top} vs {tabs}");
    assert!(top < f64::from(tabs) + 48.0, "where the toolbar would be: {top} vs {tabs}");
    remove_fixture(&dir);
}
