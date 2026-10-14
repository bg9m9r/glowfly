//! The newer keyboard shortcuts, matched through the binding table (`keymap.rs`): S/U stage or
//! unstage the open working-tree file or its selected lines, Ctrl+Shift+S/U stage or unstage
//! all, Ctrl+Shift+M goes to the commit message and Ctrl+/ opens or closes the shortcut sheet
//! (drawn by `sheet.rs`). `update/shortcuts.rs` decides whether each applies. The tab and
//! undo keys are matched here too. "Ctrl" is Cmd on macOS, except where a chord says `ctrl`.

#[cfg(test)]
mod platform_tests;
mod sheet;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;

pub(super) use sheet::show as sheet;

use crate::keymap::{self, BindingId, Chord, KeyAction, KeyName};
use crate::msg::Msg;
use crate::state::{AppState, Modal};
use eframe::egui::{self, Key, Modifiers};

/// Turns this frame's presses of the newer keys into `Msg::Shortcut`. Nothing reaches them under
/// Preferences, a credential prompt, the palette or a dialog, except Ctrl+/ closing the sheet.
/// Ctrl+/ and Ctrl+Shift+M also work while a text field has focus; the rest don't.
pub(super) fn keys(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    if st.prefs.is_some() || st.askpass.showing() || st.palette.is_some() {
        return;
    }
    let sheet = matches!(st.modal, Some(Modal::Shortcuts { .. }));
    if st.modal.is_some() && !sheet {
        return;
    }
    let mut push = |a: KeyAction| msgs.push(Msg::Shortcut(a));
    let toggle = ctx.input_mut(|i| pressed(i, BindingId::Sheet));
    if toggle {
        push(KeyAction::ToggleSheet);
    }
    if sheet {
        return;
    }
    if ctx.input_mut(|i| pressed(i, BindingId::FocusMessage)) {
        push(KeyAction::FocusMessage);
    }
    if ctx.wants_keyboard_input() {
        return;
    }
    let working = st.diff.as_ref().is_some_and(|(t, _)| t.is_working_tree());
    ctx.input_mut(|i| {
        // Shifted chords first, as elsewhere: consume_key ignores an extra Shift (so Shift+S
        // still stages), and Ctrl never matches a chord without it.
        if pressed(i, BindingId::StageAll) {
            push(KeyAction::StageAll);
        }
        if pressed(i, BindingId::UnstageAll) {
            push(KeyAction::UnstageAll);
        }
        if working && pressed(i, BindingId::Stage) {
            push(KeyAction::Stage);
        }
        if working && pressed(i, BindingId::Unstage) {
            push(KeyAction::Unstage);
        }
    });
}

/// New tab, close tab, reopen, next/previous tab, go to tab 1–8 and the last tab: Ctrl, or Cmd
/// on macOS, except Ctrl+(Shift+)Tab on every system (Cmd+Tab switches apps on macOS).
pub(super) fn tab_keys(ctx: &egui::Context, count: usize, msgs: &mut Vec<Msg>) {
    const DIGITS: [Key; 8] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8];
    ctx.input_mut(|i| {
        // Shifted chords first: consume_key ignores an extra Shift.
        if pressed(i, BindingId::ReopenTab) {
            msgs.push(Msg::ReopenClosedTab);
        } else if pressed(i, BindingId::NewTab) {
            msgs.push(Msg::NewTab);
        }
        if pressed(i, BindingId::CloseTab) {
            msgs.push(Msg::CloseTab(usize::MAX));
        }
        if pressed(i, BindingId::PrevTab) {
            msgs.push(Msg::CycleTab(-1));
        } else if pressed(i, BindingId::NextTab) {
            msgs.push(Msg::CycleTab(1));
        }
        // `GoToTab` is a digit range, which `pressed` never matches.
        for (n, key) in DIGITS.into_iter().enumerate() {
            if i.consume_key(Modifiers::COMMAND, key) {
                msgs.push(Msg::SelectTab(n));
            }
        }
        if pressed(i, BindingId::LastTab) {
            msgs.push(Msg::SelectTab(count.saturating_sub(1)));
        }
    });
}

/// Undo, and redo (two chords).
pub(super) fn undo_keys(ctx: &egui::Context, msgs: &mut Vec<Msg>) {
    ctx.input_mut(|i| {
        // Redo first: Cmd+Shift+Z would otherwise also match Undo's Cmd+Z.
        if pressed(i, BindingId::Redo) {
            msgs.push(Msg::Redo);
        } else if pressed(i, BindingId::Undo) {
            msgs.push(Msg::Undo);
        }
    });
}

/// Consumes a press of any of `id`'s chords. consume_key tolerates an extra Shift (a layout may
/// need it for "/") and an extra Alt; Alt isn't tolerated here, so Alt+S doesn't stage.
pub(super) fn pressed(i: &mut egui::InputState, id: BindingId) -> bool {
    let alt = i.modifiers.alt;
    keymap::binding(id).chords.iter().any(|c| (c.mods.alt || !alt) && egui_chord(*c).is_some_and(|(m, k)| i.consume_key(m, k)))
}

/// The egui modifiers and key for `c`; None for a digit range, which is never matched.
fn egui_chord(c: Chord) -> Option<(Modifiers, Key)> {
    // `command` is egui's Ctrl-or-Cmd; `ctrl` is Ctrl on every platform.
    let m = Modifiers { alt: c.mods.alt, ctrl: c.mods.ctrl, shift: c.mods.shift, mac_cmd: false, command: c.mods.command };
    let key = match c.key {
        KeyName::Letter(ch) => Key::from_name(&ch.to_ascii_uppercase().to_string())?,
        KeyName::Digit(n) => Key::from_name(&n.to_string())?,
        KeyName::F(n) => Key::from_name(&format!("F{n}"))?,
        KeyName::DigitRange(..) => return None,
        KeyName::Enter => Key::Enter,
        KeyName::Escape => Key::Escape,
        KeyName::Tab => Key::Tab,
        KeyName::Backspace => Key::Backspace,
        KeyName::Up => Key::ArrowUp,
        KeyName::Down => Key::ArrowDown,
        KeyName::Comma => Key::Comma,
        KeyName::Slash => Key::Slash,
        KeyName::Backtick => Key::Backtick,
    };
    Some((m, key))
}
