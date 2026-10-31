//! Preferences > Keyboard: recording a key for an action, checking it (`keymap::check`), settling
//! a clash by swapping or taking the key, removing keys and resetting them. Every change goes
//! into `st.keymap` at once (matching, hints and tooltips read it) and is saved as the changed
//! bindings in `settings.keybindings`.

use crate::effect::Effect;
use crate::keymap::{self, Check, binding, fold_ctrl};
use crate::msg::KeysMsg;
use crate::state::keys_page::{KeysPage, Pending, Recording};
use crate::state::{AppState, PrefPage};

pub(super) fn handle(st: &mut AppState, msg: KeysMsg, fx: &mut Vec<Effect>) {
    match msg {
        KeysMsg::Record { id, slot } => {
            if !binding(id).rebindable {
                return;
            }
            if let Some(p) = page(st) {
                p.recording = Some(Recording { id, slot });
                p.pending = None;
                p.confirm_reset_all = false;
            }
        }
        KeysMsg::CancelRecord => {
            if let Some(p) = page(st) {
                p.recording = None;
            }
        }
        KeysMsg::Recorded(chord) => recorded(st, chord, fx),
        KeysMsg::Swap => {
            let Some(Pending::Clash { id, slot, chord, other, fixed: false, swap: Some(swap) }) = pending(st) else { return };
            let theirs = st.keymap.chords(other).iter().map(|&k| if keymap::same_keys(k, chord, fold_ctrl()) { swap } else { k }).collect();
            st.keymap.set(other, theirs);
            let mine = keymap::apply(&st.keymap, id, slot, chord);
            st.keymap.set(id, mine);
            clear_pending(st);
            persist(st, fx);
        }
        KeysMsg::UseHere => {
            let Some(Pending::Clash { id, slot, chord, other, fixed: false, .. }) = pending(st) else { return };
            let theirs = st.keymap.chords(other).iter().copied().filter(|&k| !keymap::same_keys(k, chord, fold_ctrl())).collect();
            st.keymap.set(other, theirs);
            let mine = keymap::apply(&st.keymap, id, slot, chord);
            st.keymap.set(id, mine);
            clear_pending(st);
            persist(st, fx);
        }
        KeysMsg::DismissPending => clear_pending(st),
        KeysMsg::Remove { id, index } => {
            if !binding(id).rebindable || index >= st.keymap.chords(id).len() {
                return;
            }
            let mut chords = st.keymap.chords(id).to_vec();
            chords.remove(index);
            st.keymap.set(id, chords);
            if let Some(p) = page(st) {
                p.recording = None;
                p.pending = None;
            }
            persist(st, fx);
        }
        KeysMsg::Reset(id) => {
            if !binding(id).rebindable {
                return;
            }
            let taken = keymap::reset_taking(&mut st.keymap, id, fold_ctrl());
            let notice = (!taken.is_empty()).then(|| {
                let lines: Vec<String> =
                    taken.iter().map(|(from, c)| format!("{} was taken from {}.", c.text(), binding(*from).label)).collect();
                Pending::Notice { id, text: lines.join(" ") }
            });
            if let Some(p) = page(st) {
                p.recording = None;
                p.pending = notice;
            }
            persist(st, fx);
        }
        KeysMsg::AskResetAll => set_confirm(st, true),
        KeysMsg::CancelResetAll => set_confirm(st, false),
        KeysMsg::ResetAll => {
            st.keymap.reset_all();
            if let Some(p) = page(st) {
                p.confirm_reset_all = false;
                p.pending = None;
                p.recording = None;
            }
            persist(st, fx);
        }
    }
}

/// The key pressed while recording: applied, or held as a warning under the row.
fn recorded(st: &mut AppState, chord: keymap::Chord, fx: &mut Vec<Effect>) {
    let Some(Recording { id, slot }) = page(st).and_then(|p| p.recording.take()) else { return };
    let fold = fold_ctrl();
    let pending = match keymap::check(&st.keymap, id, chord, fold) {
        Check::Same => None,
        Check::Refused(r) => Some(Pending::Refused { id, text: r.text(chord) }),
        Check::Clash { other, fixed } => {
            let swap = if fixed { None } else { keymap::swap_chord(&st.keymap, id, slot, other, fold) };
            Some(Pending::Clash { id, slot, chord, other, fixed, swap })
        }
        Check::Ok => {
            let chords = keymap::apply(&st.keymap, id, slot, chord);
            st.keymap.set(id, chords);
            persist(st, fx);
            None
        }
    };
    if let Some(p) = page(st) {
        p.pending = pending;
    }
}

/// The Keyboard page's state, while Preferences shows it.
fn page(st: &mut AppState) -> Option<&mut KeysPage> {
    st.prefs.as_mut().filter(|p| p.page == PrefPage::Keyboard).map(|p| &mut p.keys)
}

fn pending(st: &mut AppState) -> Option<Pending> {
    page(st).and_then(|p| p.pending.clone())
}

fn clear_pending(st: &mut AppState) {
    if let Some(p) = page(st) {
        p.pending = None;
    }
}

/// Asks for (or drops) the Reset all confirmation; there is nothing to ask about at the defaults.
fn set_confirm(st: &mut AppState, on: bool) {
    let on = on && st.keymap.customized();
    if let Some(p) = page(st) {
        p.confirm_reset_all = on;
        if on {
            p.recording = None;
            p.pending = None;
        }
    }
}

/// Saves the changed bindings with the other settings.
fn persist(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.settings.keybindings = st.keymap.overrides();
    fx.extend(super::prefs::save_settings(st));
}
