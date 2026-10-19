//! Resizable graph columns: custom widths, dragging a header divider (live preview, one message
//! on release), double-clicking to reset, and grips only for shown columns. Shares the fixture
//! repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::{GraphMsg, Msg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ResizableColumn::*;
use trellis_core::settings::{ColumnWidths, ThemeChoice};

pub(super) type H = Harness<'static, (AppState, Vec<Msg>)>;

pub(super) fn pointer(h: &mut H, pos: Pos2, button: Option<bool>) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    if let Some(pressed) = button {
        events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
}

/// The centre of the grip labelled `label`.
pub(super) fn grip(h: &H, label: &str) -> Pos2 {
    let b = h.get_by_label(label).bounding_box().expect("the grip has bounds");
    pos2((b.x0 + b.x1) as f32 / 2.0, (b.y0 + b.y1) as f32 / 2.0)
}

#[test]
fn graph_columns_resized() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "columns-resized");
    // Wider BRANCH / TAG, narrower commit and date, all still leaving a message column.
    st.settings.column_widths = ColumnWidths { refs: 220.0, commit: 110.0, date: 100.0, ..ColumnWidths::default() };
    render("graph_columns_resized", st, true);
    remove_fixture(&dir);
}

#[test]
fn graph_column_drag() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "columns-drag");
    // A narrower BRANCH / TAG column leaves the message column some room to give.
    st.settings.column_widths = ColumnWidths { refs: 170.0, ..ColumnWidths::default() };
    let mut h = harness(st, false);
    h.run();
    let start = grip(&h, "Resize commit column");
    pointer(&mut h, start, Some(true));
    h.step();
    let drag_to = |h: &mut H, dx: f32| {
        pointer(h, start - egui::vec2(dx, 0.0), None);
        h.step();
    };
    for dx in [8.0, 12.0, 16.0, 20.0] {
        drag_to(&mut h, dx);
    }
    h.run();
    // Dragging the commit divider left widens the column, following the pointer.
    let drag = h.state().0.view.column_drag.expect("a drag is in progress");
    assert_eq!((drag.col, drag.start_width, drag.width), (Commit, 170.0, 190.0));
    for dx in [50.0, 60.0] {
        drag_to(&mut h, dx);
    }
    h.run();
    // In this window the message column has less than 50 px to give before its minimum width.
    let drag = h.state().0.view.column_drag.expect("a drag is in progress");
    assert!(drag.room < 50.0, "{drag:?}");
    let expected = 170.0 + drag.room.round();
    assert_eq!(drag.width, expected);
    assert!(h.state().1.iter().all(|m| !matches!(m, Msg::Graph(GraphMsg::ResizeColumn { .. }))), "nothing is sent while dragging");
    h.check_snapshot("graph_column_drag_light");

    pointer(&mut h, start - egui::vec2(60.0, 0.0), Some(false));
    h.run();
    let resized: Vec<_> = h
        .state()
        .1
        .iter()
        .filter_map(|m| if let Msg::Graph(GraphMsg::ResizeColumn { col, width }) = m { Some((*col, *width)) } else { None })
        .collect();
    assert_eq!(resized, [(Commit, expected)]);
    assert!(h.state().0.view.column_drag.is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_column_double_click_resets() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "columns-reset");
    let mut h = harness(st, true);
    h.run();
    let at = grip(&h, "Resize date column");
    // Both clicks in one frame: each harness step is a quarter second, past the double-click delay.
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(at));
    for pressed in [true, false, true, false] {
        events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::Graph(GraphMsg::ResetColumnWidth(Date)))), "a double-click resets the column");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Graph(GraphMsg::ResizeColumn { .. }))), "and doesn't resize it");
    assert!(h.state().0.view.column_drag.is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_grips_follow_visible_columns() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "columns-grips");
    st.settings.columns.tree = false;
    st.settings.columns.changes = false;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Resize branch / tag column").is_some());
    assert!(h.query_by_label("Resize commit column").is_some());
    assert!(h.query_by_label("Resize SHA column").is_none(), "the SHA is part of the commit column");
    assert!(h.query_by_label("Resize changes column").is_none(), "a hidden column has no grip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn layout_resets_restore_column_widths() {
    use crate::state::{PrefPage, PrefsState};
    for button in ["Reset to default layout", "Reset to compact layout"] {
        let mut st = AppState::new(super::snapshot_tests::settings(ThemeChoice::Dark), Vec::new(), std::time::Instant::now());
        st.settings.column_widths = ColumnWidths { refs: 300.0, date: 90.0, ..ColumnWidths::default() };
        st.prefs = Some(PrefsState::new(PrefPage::Ui));
        let mut h = harness(st, true);
        h.run();
        assert!(h.query_by_label_contains("double-click a divider").is_some(), "the help explains resizing");
        h.get_by_label(button).click();
        h.run();
        assert_eq!(h.state().0.settings.column_widths, ColumnWidths::default(), "{button}");
    }
}
