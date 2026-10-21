//! Recording a key for Preferences > Keyboard: while a row waits for a key, every key press
//! and typed text of the frame is taken before any shortcut, dialog or text field sees it. The
//! first press becomes the recorded chord (Esc cancels); keys Glowfly can't bind are swallowed
//! and recording goes on.

use super::chords::{chord_from_press, mac_press};
use crate::msg::{KeysMsg, Msg};
use crate::state::{AppState, PrefPage};
use eframe::egui::{self, Event, Key};

/// Takes this frame's keys for the recording, if one is going on; returns whether it is.
pub(super) fn keys(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) -> bool {
    let recording = st.prefs.as_ref().is_some_and(|p| p.page == PrefPage::Keyboard && p.keys.recording.is_some());
    if !recording {
        return false;
    }
    let mut first = None;
    ctx.input_mut(|i| {
        i.events.retain(|e| match e {
            Event::Key { key, pressed, repeat, modifiers, .. } => {
                if *pressed && !*repeat && first.is_none() {
                    first =
                        if *key == Key::Escape { Some(None) } else { chord_from_press(*key, *modifiers, mac_press(*modifiers)).map(Some) };
                }
                false
            }
            Event::Text(_) => false,
            _ => true,
        });
    });
    match first {
        Some(Some(chord)) => msgs.push(Msg::Keys(KeysMsg::Recorded(chord))),
        Some(None) => msgs.push(Msg::Keys(KeysMsg::CancelRecord)),
        None => {}
    }
    true
}
