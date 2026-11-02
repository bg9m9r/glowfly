//! App shortcuts under each platform's modifier set. egui reports Ctrl on Windows and Linux as
//! `ctrl` + `command`, and on macOS Cmd as `mac_cmd` + `command` and Ctrl as `ctrl` alone, so
//! these run anywhere: Cmd-or-Ctrl chords answer to the first two, Ctrl-only ones (tab cycling,
//! the terminal) to real Ctrl on both.

use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::TabsMsg;
use crate::state::{AppState, Tab};
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::settings::Settings;
use glowfly_model::terminal::TerminalMsg;
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
}

fn harness() -> Harness<'static, Keys> {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    // Three tabs, so the last one is tab 2.
    st.tabs = vec![Tab::empty(), Tab::empty(), Tab::empty()];
    let mut h = crate::ui::snapshot_check::builder()
        .build_state(|ctx, k: &mut Keys| super::input(ctx, &mut k.st, &mut k.msgs), Keys { st, msgs: Vec::new() });
    h.run();
    h
}

fn name(m: &Msg) -> String {
    match m {
        Msg::Tabs(TabsMsg::NewTab) => "new tab".into(),
        Msg::Tabs(TabsMsg::ReopenClosedTab) => "reopen tab".into(),
        Msg::Tabs(TabsMsg::CloseTab(_)) => "close tab".into(),
        Msg::Tabs(TabsMsg::CycleTab(d)) => format!("cycle {d}"),
        Msg::Tabs(TabsMsg::SelectTab(n)) => format!("tab {n}"),
        Msg::Job(JobMsg::Undo) => "undo".into(),
        Msg::Job(JobMsg::Redo) => "redo".into(),
        Msg::Terminal(TerminalMsg::Toggle) => "terminal".into(),
        Msg::Shortcut(a) => format!("{a:?}"),
        _ => "other".into(),
    }
}

/// What one press of `key` with `modifiers` does.
fn press(h: &mut Harness<'static, Keys>, modifiers: Modifiers, key: Key) -> Vec<String> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs).iter().map(name).collect()
}

#[test]
fn cmd_or_ctrl_chords_follow_the_platform() {
    let mut h = harness();
    // The primary modifier on each platform: Ctrl on Windows/Linux, Cmd on macOS.
    for m in [PC_CTRL, MAC_CMD] {
        assert_eq!(press(&mut h, m, Key::T), ["new tab"], "{m:?}");
        assert_eq!(press(&mut h, shift(m), Key::T), ["reopen tab"], "{m:?}");
        assert_eq!(press(&mut h, m, Key::W), ["close tab"], "{m:?}");
        assert_eq!(press(&mut h, m, Key::Num2), ["tab 1"], "{m:?}");
        assert_eq!(press(&mut h, m, Key::Num9), ["tab 2"], "the last tab, {m:?}");
        assert_eq!(press(&mut h, m, Key::Z), ["undo"], "{m:?}");
        assert_eq!(press(&mut h, shift(m), Key::Z), ["redo"], "{m:?}");
        assert_eq!(press(&mut h, m, Key::Y), ["redo"], "{m:?}");
        assert_eq!(press(&mut h, shift(m), Key::S), ["StageAll"], "{m:?}");
        assert_eq!(press(&mut h, m, Key::Slash), ["ToggleSheet"], "{m:?}");
    }
    // macOS Ctrl is not Cmd: none of those answer to it.
    for (m, key) in [(MAC_CTRL, Key::T), (MAC_CTRL, Key::W), (MAC_CTRL, Key::Z), (shift(MAC_CTRL), Key::S)] {
        assert_eq!(press(&mut h, m, key), Vec::<String>::new(), "{m:?} {key:?}");
    }
    // Control+digit is the pane key on macOS (elsewhere the pane keys are Alt+digit).
    let pane: Vec<String> = if cfg!(target_os = "macos") { vec!["FocusPane(Central)".into()] } else { Vec::new() };
    assert_eq!(press(&mut h, MAC_CTRL, Key::Num2), pane);
}

#[test]
fn ctrl_only_chords_want_real_ctrl_everywhere() {
    let mut h = harness();
    for m in [PC_CTRL, MAC_CTRL] {
        assert_eq!(press(&mut h, m, Key::Tab), ["cycle 1"], "{m:?}");
        assert_eq!(press(&mut h, shift(m), Key::Tab), ["cycle -1"], "{m:?}");
        assert_eq!(press(&mut h, m, Key::Backtick), ["terminal"], "{m:?}");
    }
    // Cmd+Tab switches apps and Cmd+` windows on macOS; Glowfly leaves them alone.
    assert_eq!(press(&mut h, MAC_CMD, Key::Tab), Vec::<String>::new());
    assert_eq!(press(&mut h, MAC_CMD, Key::Backtick), Vec::<String>::new());
}

#[test]
fn alt_gr_is_not_a_shortcut() {
    // Windows reports AltGr as Ctrl+Alt; AltGr+T, Z or Y types a character on some layouts.
    let mut h = harness();
    let alt_gr = Modifiers { alt: true, ..PC_CTRL };
    assert_eq!(press(&mut h, alt_gr, Key::T), Vec::<String>::new());
    assert_eq!(press(&mut h, alt_gr, Key::Z), Vec::<String>::new());
}
