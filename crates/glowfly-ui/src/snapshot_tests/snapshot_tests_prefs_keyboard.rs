//! Preferences > Keyboard: the full list, a row recording a key, a clash with its buttons next to
//! a fixed row, the Repository commands rows (no key until one is added), and the page's
//! controls only asking update (`KeysMsg`) to change anything.

use super::snapshot_check::CheckSnapshot;
use super::snapshot_tests::{harness, render, settings};
use crate::keymap::{BindingId as I, Chord, KeyName as K, Slot};
use crate::msg::{KeysMsg, Msg};
use crate::state::keys_page::{Pending, Recording};
use crate::state::{AppState, PrefPage, PrefsState};
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

fn page(theme: ThemeChoice, filter: &str) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Keyboard);
    prefs.keys.filter = filter.into();
    st.prefs = Some(prefs);
    st
}

fn keys_page(st: &mut AppState) -> &mut crate::state::keys_page::KeysPage {
    &mut st.prefs.as_mut().unwrap().keys
}

#[test]
fn prefs_keyboard() {
    render("prefs_keyboard", page(ThemeChoice::Dark, ""), true);
}

/// The Repository commands rows read "No key" with Add until one gets a key.
#[test]
fn prefs_keyboard_commands() {
    let mut st = page(ThemeChoice::Dark, "repository commands");
    st.keymap.set(I::Fetch, vec![Chord::plain(K::F(8))]);
    let mut h = harness(st, true);
    h.run();
    for label in ["Repository commands", "Fetch all remotes", "Open the repository in a terminal", "Change F8 for Fetch all remotes"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_all_by_label("Push").next().is_some());
    assert_eq!(h.query_all_by_label("Reset").count(), 1, "only Fetch differs from its default");
    assert_eq!(h.query_all_by_label("No key").count(), 15);
    h.check_snapshot("prefs_keyboard_commands");
}

#[test]
fn prefs_keyboard_favorites() {
    let mut h = harness(page(ThemeChoice::Dark, "favorite"), true);
    h.run();
    assert!(h.query_by_label("Repositories").is_some(), "the group");
    assert!(h.query_by_label("Open favorite repository 9").is_some());
    h.check_snapshot("prefs_keyboard_favorites");
}

/// A key's × strengthens on its hover fill and says what it does.
#[test]
fn prefs_keyboard_remove_hover() {
    let mut h = harness(page(ThemeChoice::Dark, "tab"), true);
    h.run();
    h.query_all_by_label("Remove this key").next().expect("a key with its ×").hover();
    h.run();
    h.run();
    h.check_snapshot("prefs_keyboard_remove_hover");
}

#[test]
fn prefs_keyboard_recording() {
    let mut st = page(ThemeChoice::Dark, "tab");
    st.keymap.set(I::NewTab, vec![Chord::cmd_shift(K::Letter('N'))]);
    keys_page(&mut st).recording = Some(Recording { id: I::CloseTab, slot: Slot::Replace(0) });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_keyboard_recording");
    assert!(h.query_by_label("Press keys…").is_some());
    assert!(h.query_by_label("Esc cancels").is_some());
    assert!(h.query_by_label("Reset").is_some(), "New tab differs from its default");
}

#[test]
fn prefs_keyboard_clash() {
    let mut st = page(ThemeChoice::Light, "tab");
    let chord = Chord::cmd(K::Letter('T'));
    let swap = Some(Chord::cmd_shift(K::Letter('T')));
    keys_page(&mut st).pending =
        Some(Pending::Clash { id: I::ReopenTab, slot: Slot::Replace(0), chord, other: I::NewTab, fixed: false, swap });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("prefs_keyboard_clash");
    for label in ["Swap", "Use here", "Cancel", "Fixed", "Go to tab 1 to 8"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
}

/// The messages clicking `label` sends.
fn click(st: AppState, label: &str) -> Vec<Msg> {
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

fn keys_msgs(msgs: &[Msg]) -> Vec<&KeysMsg> {
    msgs.iter().filter_map(|m| if let Msg::Keys(k) = m { Some(k) } else { None }).collect()
}

#[cfg(not(target_os = "macos"))]
#[test]
fn row_controls_only_ask() {
    let msgs = click(page(ThemeChoice::Dark, "new tab"), "Change Ctrl+T for New tab");
    assert!(matches!(keys_msgs(&msgs)[..], [KeysMsg::Record { id: I::NewTab, slot: Slot::Replace(0) }]));
    let msgs = click(page(ThemeChoice::Dark, "new tab"), "Add");
    assert!(matches!(keys_msgs(&msgs)[..], [KeysMsg::Record { id: I::NewTab, slot: Slot::Add }]));
    let msgs = click(page(ThemeChoice::Dark, "new tab"), "Remove this key");
    assert!(matches!(keys_msgs(&msgs)[..], [KeysMsg::Remove { id: I::NewTab, index: 0 }]));

    let mut st = page(ThemeChoice::Dark, "new tab");
    st.keymap.set(I::NewTab, vec![Chord::plain(K::F(9))]);
    let msgs = click(st, "Reset");
    assert!(matches!(keys_msgs(&msgs)[..], [KeysMsg::Reset(I::NewTab)]));
}

#[test]
fn reset_all_needs_a_change() {
    let mut h = harness(page(ThemeChoice::Dark, ""), true);
    h.run();
    h.get_by_label("Reset all to defaults").click();
    h.run();
    assert!(keys_msgs(&h.state().1).is_empty(), "disabled at the defaults");
    drop(h);

    let mut st = page(ThemeChoice::Dark, "");
    st.keymap.set(I::Refresh, vec![Chord::plain(K::F(9))]);
    let msgs = click(st, "Reset all to defaults");
    assert!(matches!(keys_msgs(&msgs)[..], [KeysMsg::AskResetAll]));
}

#[test]
fn fixed_rows_have_no_controls() {
    let mut h = harness(page(ThemeChoice::Dark, "go to tab 1"), true);
    h.run();
    assert!(h.query_by_label("Go to tab 1 to 8").is_some());
    assert!(h.query_by_label("Fixed").is_some());
    for label in ["Add", "Reset", "Remove this key"] {
        assert!(h.query_by_label(label).is_none(), "{label}");
    }
    assert!(h.query_by_label_contains("Change ").is_none());
}

#[test]
fn typing_in_the_filter_narrows_the_rows() {
    let mut h = harness(page(ThemeChoice::Dark, ""), true);
    h.run();
    assert!(h.query_by_label("Refresh").is_some() && h.query_by_label("New tab").is_some());
    h.get(By::new().role(Role::TextInput)).click();
    h.run();
    h.get(By::new().role(Role::TextInput)).type_text("refresh");
    h.run();
    assert_eq!(h.state().0.prefs.as_ref().unwrap().keys.filter, "refresh");
    assert!(h.query_by_label("Refresh").is_some());
    assert!(h.query_by_label("New tab").is_none());
}

#[test]
fn command_rows_only_ask() {
    let msgs = click(page(ThemeChoice::Dark, "push"), "Add");
    assert!(matches!(keys_msgs(&msgs)[..], [KeysMsg::Record { id: I::Push, slot: Slot::Add }]));

    let mut st = page(ThemeChoice::Dark, "push");
    st.keymap.set(I::Push, vec![Chord::plain(K::F(8))]);
    let msgs = click(st, "Reset");
    assert!(matches!(keys_msgs(&msgs)[..], [KeysMsg::Reset(I::Push)]));

    let mut st = page(ThemeChoice::Dark, "push");
    st.keymap.set(I::Push, vec![Chord::plain(K::F(8))]);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Reset").hover();
    h.run();
    h.run();
    assert!(h.query_by_label("Back to no key").is_some(), "the default is no key");
}
