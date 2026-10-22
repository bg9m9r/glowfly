//! Tab bar interaction tests: dragging a tab to reorder it, the tab context menu (and why its
//! items are off with one tab and none closed), and worktree tab groups (their strip and chip,
//! dragging a group, the repository line in the tooltip).

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::msg::TabsMsg;
use crate::state::{AppState, Tab};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::CommonDir;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// The fixture's two tabs plus a third, so a tab can be dropped in the middle.
fn three_tabs(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.tabs.push(Tab::unloaded(PathBuf::from("C:/work/docs")));
    (st, dir)
}

/// The centre of tab `i` as last drawn (tabs are as wide as their names).
pub(super) fn tab_centre(h: &Harness<'_, (AppState, Vec<Msg>)>, i: usize) -> Pos2 {
    super::tabs::tab_rects(&h.ctx)[i].center()
}

pub(super) fn pointer(h: &mut Harness<'_, (AppState, Vec<Msg>)>, pos: Pos2, button: Option<(PointerButton, bool)>) {
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
    let start = tab_centre(&h, 0);
    // Grabbed at its centre and held a little past tab 1's centre.
    let over = tab_centre(&h, 1) + egui::vec2(24.0, 0.0);
    pointer(&mut h, start, Some((PointerButton::Primary, true)));
    h.step();
    pointer(&mut h, start + egui::vec2(12.0, 0.0), None);
    h.step();
    pointer(&mut h, over, None);
    h.step();
    h.run();
    assert!(h.state().1.is_empty(), "nothing is sent while dragging");
    h.check_snapshot("tabs_dragging");

    pointer(&mut h, over, Some((PointerButton::Primary, false)));
    h.run();
    let msgs = &h.state().1;
    assert!(
        msgs.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::MoveTab { from: 0, to: 1 }))),
        "dropping nearest tab 1's slot moves tab 0 to 1"
    );
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::SelectTab(_)))), "dragging does not select the tab");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tab_context_menu() {
    let (mut st, dir) = three_tabs("tab-menu");
    st.closed_tabs = vec![PathBuf::from("C:/work/old-repo").into()];
    let mut h = harness(st, true);
    h.run();
    let right_click = |h: &mut Harness<'_, (AppState, Vec<Msg>)>, i: usize| {
        let at = tab_centre(h, i);
        pointer(h, at, Some((PointerButton::Secondary, true)));
        h.step();
        pointer(h, at, Some((PointerButton::Secondary, false)));
        h.run();
    };
    right_click(&mut h, 1);
    h.check_snapshot("tabs_menu");
    h.get_by_label("Close other tabs").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::CloseOtherTabs(1)))));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::SelectTab(_)))), "right-clicking does not select the tab");

    // On the last tab, "Close tabs to the right" is disabled.
    right_click(&mut h, 2);
    h.get_by_label("Close tabs to the right").click();
    h.run();
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::CloseTabsToRight(_)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tab_menu_says_why_items_are_off() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tab-menu-reasons");
    st.tabs.truncate(1);
    st.closed_tabs.clear();
    let mut h = harness(st, true);
    h.run();
    let at = tab_centre(&h, 0);
    pointer(&mut h, at, Some((PointerButton::Secondary, true)));
    h.step();
    pointer(&mut h, at, Some((PointerButton::Secondary, false)));
    h.run();
    let t = glowfly_core::i18n::Lang::En;
    for (label, key) in [
        ("Close other tabs", "tabs.menu.close_others.blocked"),
        ("Close tabs to the right", "tabs.menu.close_right.blocked"),
        ("Reopen closed tab", "tabs.menu.reopen.blocked"),
    ] {
        let item = h.get_by_label(label);
        assert!(item.is_disabled(), "{label}");
        assert_eq!(item.description().as_deref(), Some(t.tr(key)), "{label}");
    }
    assert!(!h.get_by_label("Close tab").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

/// Five tabs: the fixture and a linked worktree of it (group A), an ungrouped tab, and two tabs
/// of group B. The linked and B tabs are unloaded, with their groups set by hand.
pub(super) fn grouped_tabs(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let base = glowfly_core::paths::temp_dir().join(format!("glowfly-tab-groups-{name}"));
    let a = st.snap.as_ref().unwrap().common.clone();
    let b = CommonDir { dir: base.join("api").join(".git"), folder: None, name: "api".into() };
    let tab = |n: &str, group: Option<&CommonDir>| Tab { group: group.cloned(), ..Tab::unloaded(base.join(n)) };
    st.tabs = vec![
        Tab { group: Some(a.clone()), ..Tab::unloaded(dir.clone()) },
        tab("demo-hotfix", Some(&a)),
        tab("notes", None),
        tab("api", Some(&b)),
        tab("api-review", Some(&b)),
    ];
    (st, dir)
}

#[test]
fn grouped_tabs_have_a_strip() {
    let (st, dir) = grouped_tabs("tabs-grouped");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tabs_grouped");
    // The chip before the first group's first tab names the group on hover.
    let slot = super::tabs::tab_rects(&h.ctx)[0];
    pointer(&mut h, pos2(slot.left() + 10.0, slot.center().y), None);
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label("demo: 2 worktree tabs").is_some(), "the chip's tooltip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dragging_a_grouped_tab_out_moves_its_group() {
    let (st, dir) = grouped_tabs("tabs-group-drag");
    let mut h = harness(st, true);
    h.run();
    let drag_to = |h: &mut Harness<'_, (AppState, Vec<Msg>)>, slot: usize, release: bool| {
        let (start, over) = (tab_centre(h, 0), tab_centre(h, slot));
        pointer(h, start, Some((PointerButton::Primary, true)));
        h.step();
        pointer(h, start + egui::vec2(12.0, 0.0), None);
        h.step();
        pointer(h, over, None);
        h.step();
        h.run();
        if release {
            pointer(h, over, Some((PointerButton::Primary, false)));
            h.run();
        }
    };
    // Past group B: both group A tabs slide (the dragged one floats over slot 3).
    let over = tab_centre(&h, 3);
    drag_to(&mut h, 3, false);
    assert!(h.state().1.is_empty(), "nothing is sent while dragging");
    h.check_snapshot("tabs_group_dragging");
    pointer(&mut h, over, Some((PointerButton::Primary, false)));
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::MoveTab { from: 0, to: 3 }))), "the raw slot is sent");

    // Inside the group: a move within it.
    h.state_mut().1.clear();
    drag_to(&mut h, 1, true);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::MoveTab { from: 0, to: 1 }))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_grouped_tab_names_its_repository_on_hover() {
    for (i, grouped) in [(1, true), (2, false)] {
        let (st, dir) = grouped_tabs(&format!("tabs-group-tip-{i}"));
        let mut h = harness(st, true);
        h.run();
        let at = tab_centre(&h, i);
        pointer(&mut h, at, None);
        for _ in 0..4 {
            h.run();
        }
        let name = if grouped { "demo-hotfix" } else { "notes" };
        assert!(h.query_by_label_contains(name).is_some(), "tab {i} shows its path");
        assert_eq!(h.query_by_label_contains("Repository: demo").is_some(), grouped, "tab {i}");
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn new_tab_plus_matches_the_close_cross() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "tabs-new-tab");
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_label("New tab").bounding_box().expect("the + has bounds");
    let (w, ht) = ((b.x1 - b.x0) as f32, (b.y1 - b.y0) as f32);
    let strip_h = super::tabs::HEIGHT - super::tab_paint::LIFT;
    assert_eq!((w, ht), (32.0, strip_h), "the click target is the whole slot");
    let centre = pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    assert!((centre.y - tab_centre(&h, 0).y).abs() <= 0.5, "the + sits on the tabs' line: {centre:?}");
    let last = super::tabs::tab_rects(&h.ctx)[1];
    assert_eq!(b.x0 as f32 - last.right(), 4.0, "4 px after the last tab");
    pointer(&mut h, centre, None);
    h.run();
    h.check_snapshot("tabs_new_tab_hover");
    pointer(&mut h, centre, Some((PointerButton::Primary, true)));
    h.step();
    pointer(&mut h, centre, Some((PointerButton::Primary, false)));
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::NewTab))), "a click opens a new tab");
    drop(h);
    remove_fixture(&dir);
}
