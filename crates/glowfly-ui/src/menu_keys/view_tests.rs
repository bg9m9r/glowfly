//! Shift+F10 on the app's rows: the selected graph commit (with nothing focused), a focused
//! sidebar row and the selected file of a focused Files list each open their menu below the
//! row with its first item focused, and Esc gives the focus back to a focused row.

use crate::msg::Msg;
use crate::state::{AppState, Selection};
use crate::ui::graph::ROW_H;
use crate::ui::snapshot_tests::snapshot_tests_line_counts::with_wip;
use crate::ui::snapshot_tests::snapshot_tests_select::{click, row_y};
use crate::ui::snapshot_tests::snapshot_tests_stash::with_stashes;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::ui::wip_keys::focused_list;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, Order, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget};
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const STASH: &str = "On main: login form tweaks";

fn shift_f10(h: &mut H) {
    h.press_key_modifiers(Modifiers::SHIFT, Key::F10);
    h.run_ok();
}

/// The focused widget's top edge, when it is in a menu (a foreground layer).
fn focus_in_menu(h: &H) -> Option<f32> {
    let f = h.ctx.memory(|m| m.focused())?;
    let r = h.ctx.read_response(f)?;
    (r.layer_id.order == Order::Foreground).then_some(r.rect.top())
}

/// Moves the pointer off the window's content, so no hover counts.
fn pointer_away(h: &mut H) {
    h.input_mut().events.push(Event::PointerMoved(pos2(1.0, 1.0)));
    h.run_ok();
}

#[test]
fn shift_f10_opens_the_selected_commits_menu_below_its_row() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "menu-keys-graph");
    let head = st.snap.as_ref().unwrap().commits[0].oid;
    st.session.selection = Selection::Commit(head);
    let mut h = harness(st, true);
    h.run_ok();
    assert!(!h.ctx.is_context_menu_open() && h.ctx.memory(|m| m.focused()).is_none());
    shift_f10(&mut h);
    assert!(h.query_by_label("Checkout (detached)").is_some(), "the selected commit's menu opened");
    let top = focus_in_menu(&h).expect("its first item has the focus");
    assert!(top >= row_y(0) + ROW_H / 2.0, "below the row");
    h.press_key(Key::Escape);
    h.run_ok();
    assert!(h.query_by_label("Checkout (detached)").is_none(), "Esc closed it");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shift_f10_opens_a_focused_sidebar_rows_menu_and_esc_gives_the_focus_back() {
    let (st, dir) = with_stashes(ThemeChoice::Dark, "menu-keys-sidebar");
    let mut h = harness(st, true);
    h.run_ok();
    let row = h.get_all_by_label(STASH).filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).unwrap();
    h.get_all_by_label(STASH).find(|n| n.bounding_box().is_some_and(|b| b.x0 == row.x0)).unwrap().focus();
    h.run_ok();
    shift_f10(&mut h);
    assert!(h.query_by_label("Apply").is_some(), "the stash's menu opened");
    let top = focus_in_menu(&h).expect("its first item has the focus");
    assert!(top >= row.y1 as f32, "below the row");
    h.press_key(Key::ArrowDown);
    h.run_ok();
    h.press_key(Key::Escape);
    h.run_ok();
    assert!(h.query_by_label("Apply").is_none(), "Esc closed it");
    assert!(h.query_all_by_label(STASH).any(|n| n.is_focused()), "the row has the focus back");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shift_f10_opens_the_selected_files_menu_while_its_list_has_the_keyboard() {
    let (mut st, dir) = with_wip(ThemeChoice::Dark, "menu-keys-file");
    // app.txt's diff open: its row is the selected one.
    let target = DiffTarget::Unstaged("app.txt".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    st.session.diff.view = Some((target, diff.into()));
    let mut h = harness(st, true);
    h.run_ok();
    let b = h.get_all_by_role_and_label(Role::Button, "app.txt").find_map(|n| n.bounding_box()).expect("the file row");
    click(&mut h, pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32), PointerButton::Primary);
    pointer_away(&mut h);
    assert_eq!(focused_list(&h.ctx), Some(false), "the Unstaged list has the keyboard");
    shift_f10(&mut h);
    assert!(h.ctx.is_context_menu_open(), "the file's menu opened");
    let top = focus_in_menu(&h).expect("its first item has the focus");
    assert!(top >= b.y1 as f32, "below the row");
    h.press_key(Key::Escape);
    h.run_ok();
    assert!(!h.ctx.is_context_menu_open(), "Esc closed it");
    drop(h);
    remove_fixture(&dir);
}
