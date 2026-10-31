//! Key presses as chords, and taking a press that one of the keymap's bindings answers to out of
//! egui's input before the views see it.

use crate::keymap::{BindingId, Chord, KeyName, Keymap, Mods};
use eframe::egui::{Event, InputState, Key, Modifiers};

/// The keymap's name for `k`; None for keys it can't bind (F13 and up, Copy, braces…).
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
/// `command` only there (off macOS Ctrl is both `ctrl` and `command`). Read from the press, not
/// the build target, so the platform tests run on every system; a press with neither modifier
/// maps to the same chord either way.
pub(crate) fn mac_press(m: Modifiers) -> bool {
    m.mac_cmd || (m.ctrl && !m.command)
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

/// Whether a press with `m` is one the focused terminal would send to the shell: no Shift, no
/// Alt and no macOS Cmd (the terminal drops Cmd presses).
pub(crate) fn reaches_shell(m: Modifiers) -> bool {
    !m.shift && !m.alt && !m.mac_cmd
}

/// Takes the first key press (repeats too) whose binding `allowed` lets through out of `i`'s
/// events, returning the binding, the pressed chord and whether the press is a key repeat.
/// `allowed` also gets whether the press would reach a focused terminal's shell
/// (`reaches_shell`). Presses of other keys, and of bindings that don't apply now, stay for the
/// views.
pub(crate) fn take_press(i: &mut InputState, keys: &Keymap, allowed: impl Fn(BindingId, bool) -> bool) -> Option<(BindingId, Chord, bool)> {
    let (n, id, chord, repeat) = i.events.iter().enumerate().find_map(|(n, e)| {
        let Event::Key { key, pressed: true, modifiers, repeat, .. } = e else { return None };
        let mac = mac_press(*modifiers);
        let chord = chord_from_press(*key, *modifiers, mac)?;
        let id = matched(keys, chord, !mac)?;
        allowed(id, reaches_shell(*modifiers)).then_some((n, id, chord, *repeat))
    })?;
    i.events.remove(n);
    Some((id, chord, repeat))
}
