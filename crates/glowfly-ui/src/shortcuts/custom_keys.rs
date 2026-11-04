//! The keys custom commands claim (`state::custom_commands::chords`), matched after the
//! built-in bindings have taken theirs, and only while nothing covers the main window.

use super::chords::{chord_from_press, mac_press};
use eframe::egui::{Event, InputState};
use glowfly_model::custom_commands::CommandRef;
use glowfly_model::keymap::same_keys;
use glowfly_model::state::custom_commands::CommandKey;

/// Takes the first key press (repeats too) that one of `keys` claims out of `i`'s events,
/// returning its command.
pub(crate) fn take(i: &mut InputState, keys: &[CommandKey]) -> Option<CommandRef> {
    let (n, r) = i.events.iter().enumerate().find_map(|(n, e)| {
        let Event::Key { key, pressed: true, modifiers, .. } = e else { return None };
        let mac = mac_press(*modifiers);
        let chord = chord_from_press(*key, *modifiers, mac)?;
        keys.iter().find(|(k, _, _)| same_keys(*k, chord, !mac)).map(|(_, r, _)| (n, r.clone()))
    })?;
    i.events.remove(n);
    Some(r)
}
