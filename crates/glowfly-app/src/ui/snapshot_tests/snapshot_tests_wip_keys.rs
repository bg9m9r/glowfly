//! The Unstaged and Staged Files lists' keyboard focus: a click on a row focuses its list, the
//! focused list rings its current row (or, with none, the whole list), the files a Shift move
//! selected are drawn selected, each of the list's keys becomes its message (taken before any
//! binding, not while a dialog covers the list or a field has the keyboard), arrows in the first
//! frame after the list took the focus keep it there, and a focused list doesn't count as typing.
//! Shares the fixture repositories with `snapshot_tests` and `snapshot_tests_line_counts`.

use super::snapshot_tests::{harness, keys_harness, remove_fixture};
use super::snapshot_tests_line_counts::with_wip;
use super::snapshot_tests_select::click;
use super::wip_keys::{focused_list, list_id};
use crate::keymap::{BindingId, Chord, KeyAction, KeyName};
use crate::msg::{DiffMsg, Msg};
use crate::state::list_keys::{ListKey, ListMove};
use crate::state::wip_lists::WipSpan;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The working tree with line counts and app.txt's unstaged diff open. Unstaged Files: README.md,
/// app.txt, login.txt, notes/todo.md; Staged Files: app.txt, search.txt.
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
    h.input_mut().events.push(Event::PointerMoved(pos2(1.0, 1.0)));
    h.run_ok();
    h.state_mut().1.clear();
}

/// The list keys the frames so far sent, with their list.
fn keys(h: &H) -> Vec<(bool, ListKey)> {
    h.state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Shortcut(KeyAction::WipList { staged, key }) => Some((*staged, *key)),
            _ => None,
        })
        .collect()
}

fn mv(m: ListMove) -> (bool, ListKey) {
    (false, ListKey::Move(m))
}

const DOWN: ListMove = ListMove::Step { down: true };
const UP: ListMove = ListMove::Step { down: false };

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
fn wip_list_selected_files() {
    // A Shift move from app.txt down to login.txt: both rows are drawn selected, the Staged
    // list's app.txt isn't.
    let (mut st, dir) = with_open(ThemeChoice::Dark, "wip-keys-span");
    let target = DiffTarget::Unstaged("login.txt".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    st.session.diff.view = Some((target, diff.into()));
    st.view.wip_span = Some(WipSpan { staged: false, anchor: "app.txt".into(), head: "login.txt".into() });
    let mut h = harness(st, true);
    h.run();
    h.ctx.memory_mut(|m| m.request_focus(list_id(false)));
    h.run_ok();
    h.check_snapshot("wip_list_selected_files_dark");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn focused_list_keys_become_their_messages() {
    let (st, dir) = with_open(ThemeChoice::Dark, "wip-keys-arrows");
    let mut h = keys_harness(st, true);
    h.run();
    assert_eq!(focused_list(&h.ctx), None);
    h.press_key(Key::ArrowDown);
    h.run_ok();
    assert!(keys(&h).is_empty(), "no list key while no list has the focus");

    click_row(&mut h, "app.txt");
    for key in [Key::ArrowDown, Key::ArrowUp, Key::Home, Key::End, Key::PageDown, Key::Space, Key::Enter, Key::Delete] {
        h.press_key(key);
    }
    h.press_key_modifiers(Modifiers::COMMAND, Key::Backspace);
    h.press_key_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    h.press_key_modifiers(Modifiers::SHIFT, Key::End);
    h.run_ok();
    let got = keys(&h);
    assert_eq!(got.len(), 11, "{got:?}");
    assert_eq!(got[..4], [mv(DOWN), mv(UP), mv(ListMove::Edge { end: false }), mv(ListMove::Edge { end: true })]);
    let (false, ListKey::Move(ListMove::Page { down: true, rows })) = got[4] else { panic!("{:?}", got[4]) };
    assert!(rows > 1, "a page is the rows in view, less one: {rows}");
    assert_eq!(
        got[5..],
        [
            (false, ListKey::Toggle),
            (false, ListKey::Open),
            (false, ListKey::Delete),
            (false, ListKey::Delete),
            (false, ListKey::Extend(DOWN)),
            (false, ListKey::Extend(ListMove::Edge { end: true })),
        ]
    );
    assert_eq!(focused_list(&h.ctx), Some(false), "the keys don't move the focus away");

    h.state_mut().1.clear();
    h.press_key_modifiers(Modifiers::ALT, Key::ArrowDown);
    h.press_key_modifiers(Modifiers::COMMAND, Key::ArrowUp);
    h.press_key_modifiers(Modifiers::SHIFT, Key::Space);
    h.run_ok();
    assert!(keys(&h).is_empty(), "other modifiers are left alone");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::JumpToChange(1)))), "Alt+Down is still the next change");

    h.state_mut().1.clear();
    h.press_key(Key::ArrowRight);
    h.press_key(Key::ArrowLeft);
    h.run_ok();
    assert_eq!(keys(&h), [(false, ListKey::Across { right: true }), (false, ListKey::Across { right: false })]);

    // A focused list isn't typing: Esc drops its focus like any widget's.
    h.press_key(Key::Escape);
    h.run_ok();
    assert_eq!(h.ctx.memory(|m| m.focused()), None);
    assert_ne!(list_id(false), list_id(true));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_key_rebound_elsewhere_still_reaches_the_focused_list() {
    // Delete for Show folded lines (a diff key, which the lists' keys don't overlap).
    let (mut st, dir) = with_open(ThemeChoice::Dark, "wip-keys-rebound");
    st.keymap.set(BindingId::OpenFold, vec![Chord::plain(KeyName::Delete)]);
    let mut h = keys_harness(st, true);
    h.run();
    h.press_key(Key::Delete);
    h.run_ok();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::OpenFoldAtTop))), "unfocused, the binding has Delete");
    click_row(&mut h, "app.txt");
    h.press_key(Key::Delete);
    h.run_ok();
    assert_eq!(keys(&h), [(false, ListKey::Delete)]);
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::OpenFoldAtTop))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn no_list_keys_under_a_dialog_or_while_typing() {
    let (st, dir) = with_open(ThemeChoice::Dark, "wip-keys-covered");
    let mut h = keys_harness(st, true);
    h.run();
    click_row(&mut h, "app.txt");
    h.state_mut().0.modal = Some(Modal::Shortcuts { filter: String::new() });
    h.run_ok();
    h.state_mut().1.clear();
    for key in [Key::ArrowDown, Key::Space, Key::Enter, Key::Delete] {
        h.press_key(key);
    }
    h.run_ok();
    assert!(keys(&h).is_empty(), "the sheet covers the list: {:?}", keys(&h));

    h.state_mut().0.modal = None;
    h.state_mut().0.view.focus_message = true;
    h.run_ok();
    assert!(h.state().0.view.was_typing, "the commit summary has the keyboard");
    h.state_mut().1.clear();
    h.press_key(Key::ArrowDown);
    h.press_key(Key::Space);
    h.run_ok();
    assert!(keys(&h).is_empty(), "typing: {:?}", keys(&h));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn arrows_on_the_first_frame_with_the_focus_keep_it() {
    let (st, dir) = with_open(ThemeChoice::Dark, "wip-keys-first-frame");
    let mut h = keys_harness(st, true);
    h.run();
    // The focus arrives between two frames (a click, or Tab), and Down comes with the next one.
    h.ctx.memory_mut(|m| m.request_focus(list_id(false)));
    h.press_key(Key::ArrowDown);
    h.step();
    h.run_ok();
    assert_eq!(keys(&h), [mv(DOWN)]);
    assert_eq!(focused_list(&h.ctx), Some(false), "egui's own arrow move didn't carry the focus off");
    // The next press finds the lock in place.
    h.state_mut().1.clear();
    h.press_key(Key::ArrowUp);
    h.step();
    assert_eq!(keys(&h), [mv(UP)]);
    assert_eq!(focused_list(&h.ctx), Some(false));
    drop(h);
    remove_fixture(&dir);
}
