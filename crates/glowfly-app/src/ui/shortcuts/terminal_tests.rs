//! A focused terminal keeps the unshifted keys of the tab, palette, search and refresh actions
//! for its shell, by binding and press, so rebinding moves the rule with the key. Shifted keys,
//! the terminal toggle, zoom and macOS Cmd chords still act.

use crate::keymap::{BindingId as I, Chord, KeyName as K};
use crate::msg::{Msg, PaletteMsg, RepoMsg, TabsMsg};
use crate::state::{AppState, Tab};
use crate::terminal::TerminalMsg;
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::settings::Settings;
use std::time::Instant;

/// Ctrl on Windows and Linux.
const PC_CTRL: Modifiers = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
/// Cmd on macOS.
const MAC_CMD: Modifiers = Modifiers { mac_cmd: true, command: true, ..Modifiers::NONE };
/// Ctrl on macOS.
const MAC_CTRL: Modifiers = Modifiers::CTRL;

fn shift(m: Modifiers) -> Modifiers {
    Modifiers { shift: true, ..m }
}

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// The key presses left in egui's input for the views after the shortcuts ran.
    left: Vec<Key>,
}

type H = Harness<'static, Keys>;

fn harness(focused: bool, rebind: impl FnOnce(&mut AppState)) -> H {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    st.tabs = vec![Tab::empty(), Tab::empty(), Tab::empty()];
    st.terminal.focused = focused;
    rebind(&mut st);
    let keys = Keys { st, msgs: Vec::new(), left: Vec::new() };
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
        },
        keys,
    );
    h.run();
    h
}

fn name(m: &Msg) -> String {
    match m {
        Msg::Tabs(TabsMsg::NewTab) => "new tab".into(),
        Msg::Tabs(TabsMsg::ReopenClosedTab) => "reopen tab".into(),
        Msg::Tabs(TabsMsg::CloseTab(_)) => "close tab".into(),
        Msg::Tabs(TabsMsg::CycleTab(d)) => format!("cycle {d}"),
        Msg::Palette(PaletteMsg::TogglePalette) => "palette".into(),
        Msg::Repo(RepoMsg::Refresh) => "refresh".into(),
        Msg::Terminal(TerminalMsg::Toggle) => "terminal".into(),
        Msg::Layout(crate::msg::LayoutMsg::ZoomIn) => "zoom in".into(),
        _ => "other".into(),
    }
}

/// What one press does, and whether its key was left for the views.
fn press(h: &mut H, modifiers: Modifiers, key: Key) -> (Vec<String>, bool) {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    let msgs = std::mem::take(&mut h.state_mut().msgs).iter().map(name).collect();
    (msgs, h.state().left.contains(&key))
}

/// The press does nothing in the app and is left for the terminal.
fn kept(h: &mut H, m: Modifiers, key: Key) {
    assert_eq!(press(h, m, key), (Vec::new(), true), "{m:?} {key:?}");
}

/// The press does `what` and is taken from the views.
fn acts(h: &mut H, m: Modifiers, key: Key, what: &str) {
    assert_eq!(press(h, m, key), (vec![what.to_string()], false), "{m:?} {key:?}");
}

#[test]
fn the_focused_terminal_keeps_unshifted_keys() {
    let mut h = harness(true, |_| {});
    for key in [Key::T, Key::W, Key::Tab, Key::Num1, Key::Num8, Key::Num9, Key::P, Key::F] {
        kept(&mut h, PC_CTRL, key);
    }
    kept(&mut h, Modifiers::NONE, Key::F5);
    acts(&mut h, shift(PC_CTRL), Key::P, "palette");
    acts(&mut h, shift(PC_CTRL), Key::T, "reopen tab");
    acts(&mut h, shift(PC_CTRL), Key::Tab, "cycle -1");
    acts(&mut h, PC_CTRL, Key::Backtick, "terminal");
    acts(&mut h, PC_CTRL, Key::Equals, "zoom in");
}

#[test]
fn without_the_terminal_the_keys_act() {
    let mut h = harness(false, |_| {});
    acts(&mut h, PC_CTRL, Key::W, "close tab");
    acts(&mut h, PC_CTRL, Key::P, "palette");
    acts(&mut h, Modifiers::NONE, Key::F5, "refresh");
}

#[test]
fn macos_cmd_chords_stay_global() {
    let mut h = harness(true, |_| {});
    acts(&mut h, MAC_CMD, Key::W, "close tab");
    acts(&mut h, MAC_CMD, Key::P, "palette");
    kept(&mut h, MAC_CTRL, Key::Tab);
}

#[test]
fn the_rule_follows_rebinding() {
    let mut h = harness(true, |st| {
        st.keymap.set(I::NewTab, vec![Chord::cmd_shift(K::Letter('N'))]);
        st.keymap.set(I::Refresh, vec![Chord::cmd(K::Letter('R'))]);
    });
    acts(&mut h, shift(PC_CTRL), Key::N, "new tab");
    kept(&mut h, PC_CTRL, Key::R);
    kept(&mut h, PC_CTRL, Key::T);
}
