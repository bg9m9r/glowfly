//! The Quit key: Ctrl+Q on Windows and Linux, Cmd+Q on macOS, anywhere (over a dialog and
//! Preferences too) but under a credential prompt; a focused terminal keeps an unshifted Ctrl+Q
//! for its shell, while macOS Cmd+Q never reaches the shell and quits.

use crate::msg::{AppMsg, CredentialMsg, Msg};
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::askpass::Prompt;
use glowfly_core::settings::Settings;
use std::time::Instant;

/// Ctrl on Windows and Linux.
const PC_CTRL: Modifiers = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
/// Cmd on macOS.
const MAC_CMD: Modifiers = Modifiers { mac_cmd: true, command: true, ..Modifiers::NONE };

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
}

fn harness(change: impl FnOnce(&mut AppState)) -> Harness<'static, Keys> {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    change(&mut st);
    let mut h = crate::ui::snapshot_check::builder()
        .build_state(|ctx, k: &mut Keys| super::input(ctx, &mut k.st, &mut k.msgs), Keys { st, msgs: Vec::new() });
    h.run();
    h
}

/// Whether one press of Q with `modifiers` quits.
fn quits(h: &mut Harness<'static, Keys>, modifiers: Modifiers) -> bool {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key: Key::Q, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    let msgs = std::mem::take(&mut h.state_mut().msgs);
    msgs.iter().any(|m| matches!(m, Msg::App(AppMsg::Quit)))
}

#[test]
fn ctrl_q_or_cmd_q_quits() {
    let mut h = harness(|_| {});
    assert!(quits(&mut h, PC_CTRL));
    assert!(quits(&mut h, MAC_CMD));
    assert!(!quits(&mut h, Modifiers::NONE), "a plain Q");
}

#[test]
fn a_focused_terminal_keeps_ctrl_q_for_its_shell() {
    let mut h = harness(|st| st.terminal.focused = true);
    assert!(!quits(&mut h, PC_CTRL), "the shell's XON / quoted insert");
    assert!(quits(&mut h, MAC_CMD), "Cmd+Q never reaches the shell");
}

#[test]
fn not_under_a_credential_prompt() {
    let mut h = harness(|st| {
        let prompt = CredentialMsg::CredentialPrompt { id: 1, prompt: Prompt::new("Password: ", None) };
        crate::update::update(st, Msg::Credentials(prompt), Instant::now());
        assert!(st.askpass.showing());
    });
    assert!(!quits(&mut h, PC_CTRL));
    assert!(!quits(&mut h, MAC_CMD));
}

#[test]
fn over_a_dialog_and_preferences() {
    let mut h = harness(|st| st.modal = Some(Modal::About));
    assert!(quits(&mut h, PC_CTRL));
    let mut h = harness(|st| st.prefs = Some(PrefsState::new(PrefPage::General)));
    assert!(quits(&mut h, PC_CTRL));
}
