//! Row menus open below their row, so the row stays in view: a stash row, a submodule row and a
//! graph commit row; above the row when the menu doesn't fit below; and egui's context-menu
//! bookkeeping (open, closed by Esc) still answers for them.

use super::snapshot_tests::{harness, harness_sized, remove_fixture};
use super::snapshot_tests_select::{click, row_y};
use super::snapshot_tests_stash::with_stashes;
use super::snapshot_tests_submodules::with_submodules;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::graph::ROW_H;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const STASH: &str = "On main: login form tweaks";

/// The bounds of the leftmost node labelled `label` (a sidebar row, not its menu's header).
fn row_rect(h: &H, label: &str) -> egui::Rect {
    let r = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).expect("the row");
    egui::Rect::from_min_max(pos2(r.x0 as f32, r.y0 as f32), pos2(r.x1 as f32, r.y1 as f32))
}

fn top_of(h: &H, label: &str) -> f32 {
    h.get_by_label(label).bounding_box().expect(label).y0 as f32
}

fn bottom_of(h: &H, label: &str) -> f32 {
    h.get_by_label(label).bounding_box().expect(label).y1 as f32
}

fn right_click(h: &mut H, r: egui::Rect) {
    click(h, r.center(), PointerButton::Secondary);
    // The menu is sized on its first frame and placed on the next.
    h.run();
}

#[test]
fn row_menus_open_below_their_row() {
    let (st, dir) = with_stashes(ThemeChoice::Dark, "row-menu-stash");
    let mut h = harness(st, true);
    h.run();
    let row = row_rect(&h, STASH);
    right_click(&mut h, row);
    assert!(top_of(&h, "Apply") >= row.bottom(), "the menu starts below the row");
    assert_eq!(row_rect(&h, STASH), row, "the row is still there, uncovered");
    assert!(h.ctx.is_context_menu_open(), "egui still knows a context menu is open");
    h.press_key(egui::Key::Escape);
    h.run();
    assert!(!h.ctx.is_context_menu_open(), "Esc closes it");
    assert!(h.query_by_label("Apply").is_none());
    drop(h);

    let (st, _) = with_submodules(ThemeChoice::Dark, "row-menu-submodule");
    let mut h = harness(st, true);
    h.run();
    let row = row_rect(&h, "libs/ui");
    right_click(&mut h, row);
    assert!(top_of(&h, "Copy URL") >= row.bottom(), "the menu starts below the row");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_row_menu_opens_below_the_row() {
    let (st, dir) = super::snapshot_tests::with_repo(ThemeChoice::Dark, "row-menu-graph");
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(1)), PointerButton::Secondary);
    h.run();
    let row_bottom = row_y(1) + ROW_H / 2.0;
    assert!(top_of(&h, "Checkout (detached)") >= row_bottom, "the commit menu starts below the row");
    let menu_left = h.get_by_label("Checkout (detached)").bounding_box().unwrap().x0 as f32;
    assert!((menu_left - 700.0).abs() < 24.0, "the menu's left edge is near the pointer: {menu_left}");
    assert!(h.query_by_label("Search results page").is_some(), "the row is still found");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn row_menu_opens_above_when_it_does_not_fit_below() {
    let (st, dir) = with_stashes(ThemeChoice::Dark, "row-menu-above");
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 400.0));
    h.run();
    let row = row_rect(&h, STASH);
    right_click(&mut h, row);
    h.run();
    assert!(bottom_of(&h, "Delete…") <= row.top(), "the menu ends above the row");
    assert_eq!(row_rect(&h, STASH), row);
    h.check_snapshot("row_menu_above");
    drop(h);
    remove_fixture(&dir);
}
