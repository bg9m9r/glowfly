//! Key presses as chords, and taking a press that one of the keymap's bindings answers to out of
//! egui's input before the views see it.

use crate::keymap::{BindingId, Chord, KeyName, Keymap, Mods};
use eframe::egui::{Event, InputState, Key, Modifiers};

/// The keymap's name for `k`; None for keys it can't bind (F13 and up, Copy, Plus, braces…).
/// egui's key names are the settings-file names.
pub(crate) fn key_name(k: Key) -> Option<KeyName> {
    KeyName::parse(k.name())
}

/// The egui key for `k`; None for a digit range.
#[cfg(test)]
pub(crate) fn egui_key(k: KeyName) -> Option<Key> {
    Key::from_name(k.config())
}

/// Whether a press follows macOS's modifiers: egui reports Cmd as `mac_cmd`, and Ctrl without
/// `command` only there (off macOS Ctrl is both `ctrl` and `command`).
pub(crate) fn mac_press(m: Modifiers) -> bool {
    cfg!(target_os = "macos") || m.mac_cmd || (m.ctrl && !m.command)
}

/// The chord for a press of `key` with `m`. On macOS (`mac`) Cmd is Mod and Ctrl is Ctrl; on
/// other systems Ctrl is Mod.
pub(crate) fn chord_from_press(key: Key, m: Modifiers, mac: bool) -> Option<Chord> {
    let command = if mac { m.mac_cmd } else { m.ctrl || m.command };
    let mods = Mods { command, ctrl: mac && m.ctrl, shift: m.shift, alt: m.alt };
    Some(Chord { mods, key: key_name(key)? })
}

/// The binding a press of `chord` belongs to: the one with exactly that chord, else, when no
/// binding has it, the one with the same chord without Shift (Shift+S stages; a layout may
/// need Shift for "/"). Alt has to match.
fn matched(keys: &Keymap, chord: Chord, fold: bool) -> Option<BindingId> {
    keys.lookup(chord, fold).or_else(|| {
        let plain = Chord { mods: Mods { shift: false, ..chord.mods }, key: chord.key };
        chord.mods.shift.then(|| keys.lookup(plain, fold)).flatten()
    })
}

/// Takes the first key press (repeats too) whose binding `allowed` lets through out of `i`'s
/// events, returning the binding and the pressed chord. Presses of other keys, and of bindings
/// that don't apply now, stay for the views.
pub(crate) fn take_press(i: &mut InputState, keys: &Keymap, allowed: impl Fn(BindingId) -> bool) -> Option<(BindingId, Chord)> {
    let (n, id, chord) = i.events.iter().enumerate().find_map(|(n, e)| {
        let Event::Key { key, pressed: true, modifiers, .. } = e else { return None };
        let mac = mac_press(*modifiers);
        let chord = chord_from_press(*key, *modifiers, mac)?;
        let id = matched(keys, chord, !mac)?;
        allowed(id).then_some((n, id, chord))
    })?;
    i.events.remove(n);
    Some((id, chord))
}
