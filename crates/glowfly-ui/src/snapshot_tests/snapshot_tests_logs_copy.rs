//! Copying from the logs panel and exporting the logs: the header's More menu (Copy log, and
//! Export logs on every tab, Copy log greyed while a file is read) and an activity row's
//! right-click menu (Copy entry, Copy log).

use super::snapshot_tests::harness;
use super::snapshot_tests_activity::{session, tall};
use super::snapshot_tests_select::click;
use crate::logs::{LogKind, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::logs::LogFileView;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn center_of(h: &H, label: &str) -> egui::Pos2 {
    let r = h.get_by_label(label).bounding_box().expect(label);
    pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32)
}

fn right_click(h: &mut H, label: &str) {
    let at = center_of(h, label);
    click(h, at, PointerButton::Secondary);
    // The menu is sized on its first frame and placed on the next.
    h.run();
}

#[test]
fn logs_header_more_menu() {
    let (st, _) = session(ThemeChoice::Dark);
    let mut h = tall(st, true);
    h.get_by_label("More").click();
    h.run();
    assert!(h.query_by_label("Copy log").is_some(), "the menu is open");
    h.check_snapshot("logs_header_more_menu");
    h.get_by_label("Export logs…").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::Export))));
    assert!(h.query_by_label("Copy log").is_none(), "a pick closes the menu");
    h.get_by_label("More").click();
    h.run();
    h.get_by_label("Copy log").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::CopyLog))));
}

#[test]
fn logs_more_menu_reading() {
    let (mut st, _) = session(ThemeChoice::Light);
    st.logs.tab = LogKind::Errors;
    st.logs.file = Some(LogFileView { kind: LogKind::Errors, tail: None });
    let mut h = harness(st, false);
    // The reading line's spinner keeps repainting.
    h.run_ok();
    h.get_by_label("More").click();
    h.run_ok();
    h.check_snapshot("logs_more_menu_reading");
    h.get_by_label("Copy log").click();
    h.run_ok();
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::CopyLog))), "greyed while the log is read");
    assert!(h.query_by_label("Export logs…").is_some(), "Export logs is offered on every tab");
}

#[test]
fn activity_row_menu() {
    let (st, [push, first_auto, _]) = session(ThemeChoice::Dark);
    let mut h = tall(st, true);
    let row = h.get_by_label("Push, done, Everything up-to-date").bounding_box().expect("the row");
    right_click(&mut h, "Push, done, Everything up-to-date");
    let menu_top = h.get_by_label("Copy entry").bounding_box().expect("the menu").y0;
    assert!(menu_top >= row.y1, "the menu starts below the row");
    assert!(h.query_by_label("Copy log").is_some());
    h.check_snapshot("activity_row_menu");
    h.get_by_label("Copy entry").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::CopyEntry(id)) if *id == push)));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::Fold(_)))), "a right-click doesn't fold");

    right_click(&mut h, "Auto-fetch, done, Ran 5 times");
    h.get_by_label("Copy entry").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Logs(LogsMsg::CopyEntry(id)) if *id == first_auto)), "the group by its first run");
}
