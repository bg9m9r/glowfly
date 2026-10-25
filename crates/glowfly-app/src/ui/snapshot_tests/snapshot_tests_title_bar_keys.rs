//! The drawn Windows title bar's menu from the keyboard: Alt+letter shows that menu alone under
//! a row of the five titles with its first item focused, Left and Right switch it, Esc goes
//! back to the list of five (then closes), an Alt tap opens the list with File focused, and a
//! click on the menu button always shows the list.

use crate::state::AppState;
use crate::state::menu_keys::TopMenu;
use crate::state::window_chrome::Chrome;
use crate::ui::menu_bar::keys_tests::{H, focused, key, keys_harness, press, widest};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{remove_fixture, with_repo};
use eframe::egui::{Key, Modifiers, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

fn fixture(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.view.chrome = Chrome::Windows;
    st.settings.reduce_motion = true;
    (st, dir)
}

fn open(st: AppState) -> H {
    keys_harness(st, vec2(1440.0, 880.0))
}

fn only(h: &H) -> Option<TopMenu> {
    h.state().0.view.menu_keys.only
}

/// Whether the menu is open: its list or a single menu's title row shows File.
fn menu_open(h: &H) -> bool {
    h.query_by_label("File").is_some()
}

/// Holds Alt alone for a frame and lets go.
fn alt_tap(h: &mut H) {
    h.input_mut().modifiers = Modifiers::ALT;
    h.run_ok();
    h.input_mut().modifiers = Modifiers::NONE;
    h.run_ok();
}

#[test]
fn alt_letter_shows_that_menu_alone() {
    let (st, dir) = fixture("title-keys-single");
    let mut h = open(st);
    press(&mut h, Modifiers::ALT, Key::E);
    assert_eq!(only(&h), Some(TopMenu::Edit));
    assert!(h.query_by_label("Find commits…").is_some(), "Edit's items");
    assert!(h.query_by_label("Help").is_some(), "under the row of titles");
    assert!(focused(&h, "Undo"), "its first item focused");
    h.check_snapshot("title_bar_menu_keyboard_edit");
    key(&mut h, Key::ArrowRight);
    assert_eq!(only(&h), Some(TopMenu::View));
    assert!(h.query_by_label("Zoom in").is_some());
    assert!(focused(&h, "Zoom in"));
    key(&mut h, Key::ArrowLeft);
    key(&mut h, Key::ArrowLeft);
    key(&mut h, Key::ArrowLeft);
    assert_eq!(only(&h), Some(TopMenu::Help), "Left wraps from File to Help");
    key(&mut h, Key::ArrowRight);
    key(&mut h, Key::ArrowRight);
    key(&mut h, Key::ArrowRight);
    assert_eq!(only(&h), Some(TopMenu::View));
    // Esc: back to the list, View's row focused; Esc again closes.
    key(&mut h, Key::Escape);
    assert!(menu_open(&h));
    assert_eq!(only(&h), None);
    assert!(h.query_by_label("Zoom in").is_none(), "the list of five");
    assert!(focused(&h, "View"));
    key(&mut h, Key::Escape);
    assert!(!menu_open(&h), "closed");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn an_alt_tap_focuses_file_and_enter_shows_it() {
    let (st, dir) = fixture("title-keys-tap");
    let mut h = open(st);
    alt_tap(&mut h);
    assert!(menu_open(&h));
    assert_eq!(only(&h), None);
    assert!(focused(&h, "File"), "the File row has the focus");
    assert!(h.state().0.view.menu_keys.keyboard, "letters underlined");
    key(&mut h, Key::ArrowDown);
    assert!(focused(&h, "Edit"));
    key(&mut h, Key::Enter);
    assert_eq!(only(&h), Some(TopMenu::Edit));
    assert!(focused(&h, "Undo"));
    key(&mut h, Key::Escape);
    // A row's letter shows its menu too.
    key(&mut h, Key::H);
    assert_eq!(only(&h), Some(TopMenu::Help));
    assert!(focused(&h, "Keyboard shortcuts"));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_click_on_the_menu_button_shows_the_list() {
    let (st, dir) = fixture("title-keys-click");
    let mut h = open(st);
    press(&mut h, Modifiers::ALT, Key::W);
    assert_eq!(only(&h), Some(TopMenu::Window));
    h.get_by_label("Menu").click();
    h.run_ok();
    assert!(!menu_open(&h), "the click closes it");
    h.get_by_label("Menu").click();
    h.run_ok();
    assert!(menu_open(&h));
    assert_eq!(only(&h), None, "the list of five");
    assert!(!h.state().0.view.menu_keys.keyboard);
    assert!(widest(&h, "View").bounding_box().is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn macos_has_no_mnemonics() {
    let (mut st, dir) = fixture("title-keys-mac");
    st.view.chrome = Chrome::MacOs;
    let mut h = open(st);
    press(&mut h, Modifiers::ALT, Key::E);
    assert!(!menu_open(&h));
    drop(h);
    remove_fixture(&dir);
}
