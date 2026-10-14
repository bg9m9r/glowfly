//! Tab bar interaction tests: dragging a tab to reorder it, and the tab context menu.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::{AppState, Tab};
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;

/// The fixture's two tabs plus a third, so a tab can be dropped in the middle.
fn three_tabs(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.tabs.push(Tab::unloaded(PathBuf::from("C:/work/docs")));
    (st, dir)
}

/// The centre of tab `i`: three tabs are 220 wide, 1 apart, after a 6 margin.
fn tab_centre(i: usize) -> Pos2 {
    pos2(6.0 + 221.0 * i as f32 + 110.0, 15.0)
}

fn pointer(h: &mut Harness<'_, (AppState, Vec<Msg>)>, pos: Pos2, button: Option<(PointerButton, bool)>) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    if let Some((button, pressed)) = button {
        events.push(Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
    }
}

#[test]
fn tab_drag_moves_and_snapshots() {
    let (st, dir) = three_tabs("tab-drag");
    let mut h = harness(st, true);
    h.run();
    let start = tab_centre(0);
    pointer(&mut h, start, Some((PointerButton::Primary, true)));
    h.step();
    pointer(&mut h, start + egui::vec2(12.0, 0.0), None);
    h.step();
    let over = start + egui::vec2(272.0, 0.0);
    pointer(&mut h, over, None);
    h.step();
    h.run();
    assert!(h.state().1.is_empty(), "nothing is sent while dragging");
    h.snapshot("tabs_dragging");

    pointer(&mut h, over, Some((PointerButton::Primary, false)));
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::MoveTab { from: 0, to: 1 })), "dropping between tabs 1 and 2 moves tab 0 to 1");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::SelectTab(_))), "dragging does not select the tab");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tab_context_menu() {
    let (mut st, dir) = three_tabs("tab-menu");
    st.closed_tabs = vec![PathBuf::from("C:/work/old-repo")];
    let mut h = harness(st, true);
    h.run();
    let right_click = |h: &mut Harness<'_, (AppState, Vec<Msg>)>, i: usize| {
        pointer(h, tab_centre(i), Some((PointerButton::Secondary, true)));
        h.step();
        pointer(h, tab_centre(i), Some((PointerButton::Secondary, false)));
        h.run();
    };
    right_click(&mut h, 1);
    h.snapshot("tabs_menu");
    h.get_by_label("Close other tabs").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseOtherTabs(1))));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::SelectTab(_))), "right-clicking does not select the tab");

    // On the last tab, "Close tabs to the right" is disabled.
    right_click(&mut h, 2);
    h.get_by_label("Close tabs to the right").click();
    h.run();
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::CloseTabsToRight(_))));
    drop(h);
    remove_fixture(&dir);
}
