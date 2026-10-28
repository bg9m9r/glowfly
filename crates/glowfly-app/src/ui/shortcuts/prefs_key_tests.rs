//! Preferences' keys: Ctrl/Cmd+S saves the open page (from one of its fields too), and Esc over
//! the Save / Revert bar reverts the page, closing Preferences only once nothing is unsaved.
//! Esc leaves a dialog, an open menu or a save still being written alone.

use crate::msg::{Msg, PrefsMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::prefs_save::PendingSave;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::settings::Settings;
use std::time::Instant;

const PC_CTRL: Modifiers = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
const MAC_CMD: Modifiers = Modifiers { mac_cmd: true, command: true, ..Modifiers::NONE };

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
}

/// Preferences open on the Identity page, its name edited when `edited`.
fn identity(edited: bool) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let mut p = PrefsState::new(PrefPage::RepoIdentity);
    let git = GitConfigValues { repo_name: "Ada".into(), ..Default::default() };
    p.git_saved = Some(git.clone());
    p.git = Some(GitConfigValues { repo_name: if edited { "Grace".into() } else { "Ada".into() }, ..git });
    st.prefs = Some(p);
    st
}

fn harness(st: AppState) -> Harness<'static, Keys> {
    let mut h = crate::ui::snapshot_check::builder()
        .build_state(|ctx, k: &mut Keys| super::global::input(ctx, &mut k.st, &mut k.msgs), Keys { st, msgs: Vec::new() });
    h.run();
    h
}

/// The messages one press of `key` with `modifiers` sends.
fn press(h: &mut Harness<'static, Keys>, key: Key, modifiers: Modifiers) -> Vec<Msg> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs)
}

fn saves(msgs: &[Msg]) -> bool {
    msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::SavePage)))
}

fn reverts(msgs: &[Msg]) -> bool {
    msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::RevertPage)))
}

fn closes(msgs: &[Msg]) -> bool {
    msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::ClosePrefs)))
}

#[test]
fn ctrl_s_or_cmd_s_saves_the_page() {
    let mut h = harness(identity(true));
    assert!(saves(&press(&mut h, Key::S, PC_CTRL)));
    assert!(saves(&press(&mut h, Key::S, MAC_CMD)));
    assert!(!saves(&press(&mut h, Key::S, Modifiers::NONE)), "a plain S types");
}

#[test]
fn ctrl_s_only_in_preferences_and_not_under_a_dialog() {
    let mut h = harness(AppState::new(Settings::default(), Vec::new(), Instant::now()));
    assert!(!saves(&press(&mut h, Key::S, PC_CTRL)), "nothing to save outside Preferences");
    let mut st = identity(true);
    st.modal = Some(Modal::About);
    let mut h = harness(st);
    assert!(!saves(&press(&mut h, Key::S, PC_CTRL)));
}

#[test]
fn esc_reverts_an_edited_page_then_closes() {
    let mut h = harness(identity(true));
    let msgs = press(&mut h, Key::Escape, Modifiers::NONE);
    assert!(reverts(&msgs) && !closes(&msgs), "the bar's Revert first");
    let mut h = harness(identity(false));
    let msgs = press(&mut h, Key::Escape, Modifiers::NONE);
    assert!(closes(&msgs) && !reverts(&msgs), "nothing unsaved: Esc closes Preferences");
}

#[test]
fn esc_waits_for_a_save_being_written() {
    let mut st = identity(true);
    let p = st.prefs.as_mut().unwrap();
    p.saving.push(PendingSave { ticket: 1, page: PrefPage::RepoIdentity, written: p.git.clone() });
    let mut h = harness(st);
    let msgs = press(&mut h, Key::Escape, Modifiers::NONE);
    assert!(!reverts(&msgs) && !closes(&msgs), "{}", msgs.len());
}

#[test]
fn esc_under_a_dialog_is_the_dialog_s() {
    let mut st = identity(true);
    st.modal = Some(Modal::About);
    let mut h = harness(st);
    let msgs = press(&mut h, Key::Escape, Modifiers::NONE);
    assert!(!reverts(&msgs) && !closes(&msgs));
}

#[test]
fn the_sheet_lists_both_keys() {
    let save = crate::keymap::binding(crate::keymap::BindingId::PrefsSave);
    assert!(save.rebindable, "Ctrl/Cmd+S can be changed");
    assert_eq!(save.when.note(), Some("in Preferences"));
    let revert = crate::keymap::binding(crate::keymap::BindingId::PrefsRevert);
    assert!(!revert.rebindable, "Esc is the views' own key");
}
