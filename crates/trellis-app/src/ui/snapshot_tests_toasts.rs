//! Toasts: a plain one stacked with a behind alert whose Pull button sends `Msg::ToastAction`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::{AppState, ToastAction};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::Path;
use std::time::Instant;
use trellis_core::settings::{Corner, ThemeChoice};

/// Adds a plain toast and a behind alert for `dir`; returns the alert's id.
fn add_toasts(st: &mut AppState, dir: &Path) -> u64 {
    let now = Instant::now();
    st.toast(true, "Fetch succeeded", now);
    let action = ToastAction { label: "Pull".into(), msg: Box::new(Msg::git("Pull", &["pull", "--ff"])), workdir: Some(dir.to_path_buf()) };
    st.toast_action(true, "main is 2 commits behind origin/main", action, now)
}

#[test]
fn toast_behind_alert() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "toast-behind");
    let id = add_toasts(&mut st, &dir);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("toast_behind_alert");
    // The toast's button, drawn after the toolbar's Pull.
    h.get_all_by_label("Pull").last().unwrap().click();
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
    remove_fixture(&dir);
}
