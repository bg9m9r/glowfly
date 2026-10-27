//! The Unstaged and Staged Files lists' keyboard focus: a click on a row focuses its list, the
//! focused list rings its current row (or, with none, the whole list), plain Up/Down become
//! steps while modified arrows are left alone, and a focused list doesn't count as typing.
//! Shares the fixture repositories with `snapshot_tests` and `snapshot_tests_line_counts`.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_line_counts::with_wip;
use super::snapshot_tests_select::click;
use super::wip_keys::{focused_list, list_id};
use crate::keymap::KeyAction;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The working tree with line counts and app.txt's unstaged diff open.
fn with_open(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_wip(theme, name);
    let target = DiffTarget::Unstaged("app.txt".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    st.session.diff.view = Some((target, diff.into()));
    (st, dir)
}

/// Clicks the empty area of the row named `name` (the first, when both lists have one), then
/// moves the pointer away so no hover shows.
fn click_row(h: &mut H, name: &str) {
    let b = h.get_all_by_role_and_label(Role::Button, name).find_map(|n| n.bounding_box()).expect("the row has bounds");
    // The middle: the row's end holds Stage or Unstage while the pointer is over it.
    click(h, pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32), PointerButton::Primary);
    h.input_mut().events.push(eframe::egui::Event::PointerMoved(pos2(1.0, 1.0)));
    h.run_ok();
    h.state_mut().1.clear();
}

fn steps(h: &H) -> Vec<(bool, bool)> {
    h.state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Shortcut(KeyAction::StepFile { staged, down }) => Some((*staged, *down)),
            _ => None,
        })
        .collect()
}

fn focused_row(theme: ThemeChoice, dark: bool, name: &str) {
    let (st, dir) = with_open(theme, "wip-keys-row");
    let mut h = harness(st, dark);
    h.run();
    click_row(&mut h, "app.txt");
    assert_eq!(focused_list(&h.ctx), Some(false), "the click focused the Unstaged list");
    h.check_snapshot(name);
    drop(h);
    if !dark {
        remove_fixture(&dir);
    }
}

#[test]
fn wip_list_focused_row() {
    focused_row(ThemeChoice::Dark, true, "wip_list_focused_row_dark");
    focused_row(ThemeChoice::Light, false, "wip_list_focused_row_light");
}

#[test]
fn wip_list_focused_list() {
    // search.txt is only staged, and the open diff is unstaged: the Staged list has no current row.
    let (st, dir) = with_open(ThemeChoice::Dark, "wip-keys-list");
    let mut h = harness(st, true);
    h.run();
    click_row(&mut h, "search.txt");
    assert_eq!(focused_list(&h.ctx), Some(true), "the click focused the Staged list");
    h.check_snapshot("wip_list_focused_list_dark");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn focused_list_arrows_step_and_modified_arrows_pass() {
    let (st, dir) = with_open(ThemeChoice::Dark, "wip-keys-arrows");
    let mut h = harness(st, true);
    h.run();
    assert_eq!(focused_list(&h.ctx), None);
    h.press_key(Key::ArrowDown);
    h.run_ok();
    assert!(steps(&h).is_empty(), "no step while no list has the focus");

    click_row(&mut h, "app.txt");
    h.press_key(Key::ArrowDown);
    h.run_ok();
    h.press_key(Key::ArrowUp);
    h.run_ok();
    assert_eq!(steps(&h), [(false, true), (false, false)]);
    assert_eq!(focused_list(&h.ctx), Some(false), "the arrows don't move the focus away");

    h.state_mut().1.clear();
    h.press_key_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    h.press_key_modifiers(Modifiers::ALT, Key::ArrowDown);
    h.press_key_modifiers(Modifiers::COMMAND, Key::ArrowUp);
    h.run_ok();
    assert!(steps(&h).is_empty(), "modified arrows aren't steps");

    // A focused list isn't typing: Esc drops its focus like any widget's.
    h.press_key(Key::Escape);
    h.run_ok();
    assert_eq!(h.ctx.memory(|m| m.focused()), None);
    assert_ne!(list_id(false), list_id(true));
    drop(h);
    remove_fixture(&dir);
}
