//! The Repository commands keys: a key the user gave a palette command asks update to run it,
//! only while nothing covers the main window and no text field has focus; a held key runs it
//! once, and a focused terminal keeps the presses that reach its shell.

use crate::keymap::{BindingId as I, Chord, KeyName as K};
use crate::msg::{Msg, PaletteMsg};
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::settings::Settings;
use std::time::Instant;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// Draw a text field and keep it focused.
    field: Option<String>,
    /// The key presses left in egui's input for the views after the shortcuts ran.
    left: Vec<Key>,
}

type H = Harness<'static, Keys>;

/// A state with F8 bound to Fetch all remotes.
fn fetch_on_f8() -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    st.keymap.set(I::Fetch, vec![Chord::plain(K::F(8))]);
    st
}

fn harness(st: AppState, field: bool) -> H {
    let keys = Keys { st, msgs: Vec::new(), field: field.then(String::new), left: Vec::new() };
    let mut h = crate::ui::snapshot_check::builder().build_state(
        |ctx, k: &mut Keys| {
            super::input(ctx, &mut k.st, &mut k.msgs);
            k.left = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Key { key, pressed: true, .. } => Some(*key),
                        _ => None,
                    })
                    .collect()
            });
            if let Some(text) = k.field.as_mut() {
                egui::CentralPanel::default().show(ctx, |ui| ui.text_edit_singleline(text).request_focus());
            }
        },
        keys,
    );
    h.run();
    h
}

fn name(m: &Msg) -> String {
    match m {
        Msg::Palette(PaletteMsg::RunBoundCommand(cmd)) => format!("run {cmd:?}"),
        _ => "other".into(),
    }
}

/// What pressing `key` does; with `release` it is let go in the same frame, else it stays held
/// (egui marks the next press of a held key as a repeat).
fn press_as(h: &mut H, modifiers: Modifiers, key: Key, release: bool) -> Vec<String> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    if release {
        input.events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    }
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs).iter().map(name).collect()
}

fn press(h: &mut H, modifiers: Modifiers, key: Key) -> Vec<String> {
    press_as(h, modifiers, key, true)
}

const NONE: Modifiers = Modifiers::NONE;

fn quiet() -> Vec<String> {
    Vec::new()
}

#[test]
fn a_bound_key_runs_its_command() {
    let mut h = harness(fetch_on_f8(), false);
    assert_eq!(press(&mut h, NONE, Key::F8), ["run Fetch"]);
    assert!(!h.state().left.contains(&Key::F8), "taken from the views");
    let st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    assert_eq!(press(&mut harness(st, false), NONE, Key::F8), quiet(), "no key by default");
}

#[test]
fn not_while_typing_or_under_anything() {
    assert_eq!(press(&mut harness(fetch_on_f8(), true), NONE, Key::F8), quiet(), "a text field has focus");
    let mut st = fetch_on_f8();
    st.palette = Some(Default::default());
    assert_eq!(press(&mut harness(st, false), NONE, Key::F8), quiet(), "the palette");
    let mut st = fetch_on_f8();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    assert_eq!(press(&mut harness(st, false), NONE, Key::F8), quiet(), "Preferences");
    let mut st = fetch_on_f8();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    assert_eq!(press(&mut harness(st, false), NONE, Key::F8), quiet(), "the shortcut sheet");
}

#[test]
fn a_held_key_runs_once() {
    let mut h = harness(fetch_on_f8(), false);
    assert_eq!(press_as(&mut h, NONE, Key::F8, false), ["run Fetch"]);
    for _ in 0..3 {
        assert_eq!(press_as(&mut h, NONE, Key::F8, false), quiet(), "a repeat");
        assert!(!h.state().left.contains(&Key::F8), "the repeat is swallowed");
    }
    assert_eq!(press(&mut h, NONE, Key::F8), quiet(), "still held");
    assert_eq!(press(&mut h, NONE, Key::F8), ["run Fetch"], "pressed again after letting go");
}

#[test]
fn the_focused_terminal_keeps_what_reaches_its_shell() {
    let mut st = fetch_on_f8();
    st.terminal.focused = true;
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, NONE, Key::F8), quiet());
    assert!(h.state().left.contains(&Key::F8), "left for the shell");
    assert_eq!(press(&mut h, Modifiers::SHIFT, Key::F8), ["run Fetch"], "Shift+F8 still fetches");
}

#[test]
fn a_chord_with_modifiers_runs_its_command() {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    st.keymap.set(I::OpenTerminal, vec![Chord::cmd_alt(K::Letter('T'))]);
    let mut h = harness(st, false);
    let mod_alt = Modifiers { alt: true, ..Modifiers::COMMAND };
    assert_eq!(press(&mut h, mod_alt, Key::T), ["run OpenTerminal"]);
}
