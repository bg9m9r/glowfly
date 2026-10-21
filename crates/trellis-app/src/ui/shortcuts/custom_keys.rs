//! The keys custom commands claim (`state::custom_commands::chords`), matched after the
//! built-in bindings have taken theirs, and only while nothing covers the main window.

use super::chords::{chord_from_press, mac_press};
use crate::custom_commands::CommandRef;
use crate::keymap::same_keys;
use crate::state::custom_commands::CommandKey;
use eframe::egui::{Event, InputState};

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
