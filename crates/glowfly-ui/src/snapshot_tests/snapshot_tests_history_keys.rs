//! The file and folder history lists' keys: Up/Down, Page Up/Page Down and Home/End from the
//! first, a middle and the last entry and with none selected, a page being the rows in view less
//! one, several presses in a frame sending one selection, End stopping at the last loaded entry,
//! and the cases the list stays silent (a modified press, a focused field, button or folder file
//! list, a dialog, no entries). Shares the fixtures of `snapshot_tests_history`.

use super::file_list_keys::{focused, list_id};
use super::history::page_rows;
use super::shortcuts::text_field_focused;
use super::snapshot_tests::{harness, keys_harness, remove_fixture, with_history};
use super::snapshot_tests_history::{with_file_view, with_folder};
use super::snapshot_tests_wip_keys::{H, click_row};
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::keymap::KeyAction;
use glowfly_model::msg::{HistoryMsg, Msg};
use glowfly_model::state::list_keys::FileList;
use glowfly_model::state::{AppState, Modal};

/// Selects entry `from` (and its first file), lets a frame draw it, presses `keys` together and
/// returns the entries the next frame selected.
fn press(h: &mut H, from: Option<usize>, keys: &[Key]) -> Vec<usize> {
    set(h, from);
    h.run_ok();
    h.state_mut().1.clear();
    for k in keys {
        h.press_key(*k);
    }
    h.run_ok();
    selections(h)
}

/// The selections sent so far; clears the messages.
fn selections(h: &mut H) -> Vec<usize> {
    let sent = h.state().1.iter().filter_map(|m| if let Msg::History(HistoryMsg::SelectHistoryEntry(i)) = m { Some(*i) } else { None });
    let sent = sent.collect();
    h.state_mut().1.clear();
    sent
}

fn set(h: &mut H, selected: Option<usize>) {
    let view = h.state_mut().0.session.history.view.as_mut().unwrap();
    view.selected = selected;
    view.file = 0;
}

/// The history's entries repeated to `n` of them.
fn lengthen(st: &mut AppState, n: usize) {
    let view = st.session.history.view.as_mut().unwrap();
    view.entries = view.entries.iter().cycle().take(n).cloned().collect();
}

type Places = [(Option<usize>, [Vec<usize>; 4])];

/// Each of Page Up, Page Down, Home and End from each start in `table`.
fn check_places(h: &mut H, table: &Places) {
    let keys = [Key::PageUp, Key::PageDown, Key::Home, Key::End];
    for (from, want) in table {
        for (key, want) in keys.iter().zip(want) {
            assert_eq!(&press(h, *from, &[*key]), want, "{key:?} from {from:?}");
        }
    }
}

#[test]
fn page_rows_is_the_rows_in_view_less_one() {
    assert_eq!(page_rows(440.0), 9);
    for h in [100.0, 44.0, 0.0, -5.0] {
        assert_eq!(page_rows(h), 1, "{h}");
    }
}

#[test]
fn file_history_keys_from_each_place() {
    let (st, dir) = with_history("history-keys-places", ThemeChoice::Dark, 500, 0);
    assert_eq!(st.session.history.view.as_ref().unwrap().entries.len(), 5);
    let mut h = harness(st, true);
    h.run_ok();
    check_places(
        &mut h,
        &[
            (Some(0), [vec![], vec![4], vec![], vec![4]]),
            (Some(2), [vec![0], vec![4], vec![0], vec![4]]),
            (Some(4), [vec![0], vec![], vec![0], vec![]]),
            (None, [vec![0], vec![0], vec![0], vec![4]]),
        ],
    );
    assert_eq!(press(&mut h, Some(0), &[Key::ArrowDown]), [1]);
    assert_eq!(press(&mut h, Some(0), &[Key::ArrowUp]), Vec::<usize>::new());
    assert_eq!(press(&mut h, Some(4), &[Key::ArrowDown]), Vec::<usize>::new());
    assert_eq!(press(&mut h, None, &[Key::ArrowUp]), [0]);
    assert_eq!(press(&mut h, None, &[Key::ArrowDown]), [0]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_page_is_the_rows_in_view() {
    let (mut st, dir) = with_history("history-keys-page", ThemeChoice::Dark, 500, 0);
    lengthen(&mut st, 60);
    let mut h = harness(st, true);
    h.run_ok();
    let first = press(&mut h, Some(0), &[Key::PageDown]);
    let [p] = first[..] else { panic!("one selection, got {first:?}") };
    assert!(1 < p && p < 30, "a page is {p} rows");
    assert_eq!(press(&mut h, Some(30), &[Key::PageDown]), [30 + p]);
    assert_eq!(press(&mut h, Some(30), &[Key::PageUp]), [30 - p]);
    assert_eq!(press(&mut h, Some(59), &[Key::PageUp]), [59 - p]);
    assert_eq!(press(&mut h, Some(58), &[Key::PageDown]), [59]);
    assert_eq!(press(&mut h, Some(1), &[Key::PageUp]), [0]);
    assert_eq!(press(&mut h, Some(59), &[Key::PageDown]), Vec::<usize>::new());
    assert_eq!(press(&mut h, Some(30), &[Key::Home]), [0]);
    assert_eq!(press(&mut h, Some(30), &[Key::End]), [59]);
    assert_eq!(press(&mut h, None, &[Key::End]), [59]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_keys_in_one_frame_send_one_selection() {
    let (st, dir) = with_history("history-keys-frame", ThemeChoice::Dark, 500, 0);
    let mut h = harness(st, true);
    h.run_ok();
    assert_eq!(press(&mut h, Some(0), &[Key::ArrowDown, Key::ArrowDown]), [2]);
    assert_eq!(press(&mut h, Some(0), &[Key::End, Key::ArrowUp]), [3]);
    assert_eq!(press(&mut h, Some(0), &[Key::ArrowDown, Key::ArrowUp]), Vec::<usize>::new());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_keys_leave_modified_presses() {
    let (st, dir) = with_history("history-keys-modified", ThemeChoice::Dark, 500, 2);
    let mut h = harness(st, true);
    h.run_ok();
    for (mods, key) in [(Modifiers::SHIFT, Key::End), (Modifiers::SHIFT, Key::PageDown), (Modifiers::ALT, Key::ArrowDown)] {
        set(&mut h, Some(2));
        h.run_ok();
        h.state_mut().1.clear();
        h.press_key_modifiers(mods, key);
        h.run_ok();
        assert!(selections(&mut h).is_empty(), "{mods:?} {key:?}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_end_selects_the_last_loaded_entry() {
    let (st, dir) = with_history("history-keys-partial", ThemeChoice::Dark, 2, 0);
    let view = st.session.history.view.as_ref().unwrap();
    assert_eq!(view.entries.len(), 2);
    assert!(!view.resume.is_empty());
    let mut h = harness(st, true);
    h.run_ok();
    assert_eq!(press(&mut h, Some(0), &[Key::End]), [1]);
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::History(HistoryMsg::MoreFileHistory))));
    h.state_mut().0.session.history.view.as_mut().unwrap().loading = true;
    assert_eq!(press(&mut h, Some(0), &[Key::End]), [1]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_keys_stay_with_a_focused_field() {
    let (mut st, dir) = with_file_view("history-keys-field", ThemeChoice::Dark);
    st.session.history.view.as_mut().unwrap().nav.find.query = "words".into();
    let mut h = harness(st, true);
    h.run_ok();
    h.get(By::new().role(Role::TextInput).value("words")).focus();
    h.run_ok();
    assert!(text_field_focused(&h.ctx));
    h.state_mut().1.clear();
    for key in [Key::PageUp, Key::PageDown, Key::Home, Key::End, Key::ArrowUp, Key::ArrowDown] {
        h.press_key(key);
        h.run_ok();
        assert!(selections(&mut h).is_empty(), "{key:?}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_keys_stay_with_a_focused_button_or_under_a_dialog() {
    let (st, dir) = with_history("history-keys-button", ThemeChoice::Dark, 500, 2);
    let mut h = harness(st, true);
    h.run_ok();
    h.get_by_label("Copy SHA").focus();
    h.run_ok();
    h.state_mut().1.clear();
    for key in [Key::End, Key::PageDown] {
        h.press_key(key);
        h.run_ok();
        assert!(selections(&mut h).is_empty(), "button {key:?}");
    }
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_history("history-keys-dialog", ThemeChoice::Dark, 500, 2);
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let mut h = harness(st, true);
    h.run_ok();
    h.state_mut().1.clear();
    for key in [Key::End, Key::PageDown, Key::ArrowDown] {
        h.press_key(key);
        h.run_ok();
        assert!(selections(&mut h).is_empty(), "dialog {key:?}");
    }
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_history("history-keys-empty", ThemeChoice::Dark, 500, 2);
    let view = st.session.history.view.as_mut().unwrap();
    view.entries.clear();
    view.selected = None;
    let mut h = harness(st, true);
    h.run_ok();
    h.state_mut().1.clear();
    h.press_key(Key::End);
    h.run_ok();
    assert!(selections(&mut h).is_empty(), "no entries");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn folder_history_keys_from_each_place() {
    let (st, dir) = with_folder("history-keys-folder", ThemeChoice::Dark);
    assert_eq!(st.session.history.view.as_ref().unwrap().entries.len(), 3);
    let mut h = harness(st, true);
    h.run_ok();
    check_places(
        &mut h,
        &[
            (Some(0), [vec![], vec![2], vec![], vec![2]]),
            (Some(1), [vec![0], vec![2], vec![0], vec![2]]),
            (Some(2), [vec![0], vec![], vec![0], vec![]]),
            (None, [vec![0], vec![0], vec![0], vec![2]]),
        ],
    );
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn folder_history_file_list_keeps_its_keys() {
    let (st, dir) = with_folder("history-keys-folder-files", ThemeChoice::Dark);
    let mut h = keys_harness(st, true);
    h.run_ok();
    assert_eq!(press(&mut h, Some(0), &[Key::End]), [2]);
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Shortcut(KeyAction::FileList { .. }))));
    set(&mut h, Some(0));
    h.run_ok();
    click_row(&mut h, "lexer.rs");
    assert_eq!(focused(&h.ctx), Some(FileList::FolderHistory));
    for key in [Key::End, Key::PageDown, Key::Home, Key::ArrowDown] {
        h.press_key(key);
        h.run_ok();
    }
    let moves =
        h.state().1.iter().filter(|m| matches!(m, Msg::Shortcut(KeyAction::FileList { list: FileList::FolderHistory, .. }))).count();
    assert_eq!(moves, 4);
    assert!(selections(&mut h).is_empty(), "the file list took the keys");
    h.ctx.memory_mut(|m| m.surrender_focus(list_id(FileList::FolderHistory)));
    h.run_ok();
    assert_eq!(press(&mut h, Some(0), &[Key::End]), [2]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_keys_pass_the_shortcuts() {
    let (st, dir) = with_history("history-keys-shortcuts", ThemeChoice::Dark, 500, 2);
    let mut h = keys_harness(st, true);
    h.run_ok();
    check_places(&mut h, &[(Some(2), [vec![0], vec![4], vec![0], vec![4]])]);
    drop(h);
    remove_fixture(&dir);
}
