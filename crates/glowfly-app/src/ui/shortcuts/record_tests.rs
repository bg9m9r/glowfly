//! Recording a key for Preferences > Keyboard: the press goes only to the recording (no
//! shortcut, no Esc closing Preferences, no text for a focused field), repeats and keys Glowfly
//! can't bind are skipped, and presses become chords the same way on every system.

use super::chords::{chord_from_press, egui_key, key_name};
use crate::keymap::{BindingId as I, Chord, KeyName as K, Mods, Slot};
use crate::msg::{KeysMsg, Msg};
use crate::state::keys_page::Recording;
use crate::state::{AppState, PrefPage, PrefsState};
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::settings::Settings;
use std::time::Instant;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
}

type H = Harness<'static, Keys>;

fn harness(page: PrefPage) -> H {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(page);
    prefs.keys.recording = Some(Recording { id: I::NewTab, slot: Slot::Replace(0) });
    st.prefs = Some(prefs);
    let mut h = Harness::new_state(|ctx, k: &mut Keys| super::input(ctx, &mut k.st, &mut k.msgs), Keys { st, msgs: Vec::new() });
    h.run();
    h
}

fn press(h: &mut H, modifiers: Modifiers, key: Key, repeat: bool) -> Vec<Msg> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat, modifiers });
    input.events.push(Event::Text("k".into()));
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs)
}

fn recorded(msgs: &[Msg]) -> Option<Chord> {
    match msgs {
        [Msg::Keys(KeysMsg::Recorded(c))] => Some(*c),
        _ => None,
    }
}

const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };

#[test]
fn the_press_goes_only_to_the_recording() {
    let mut h = harness(PrefPage::Keyboard);
    assert_eq!(recorded(&press(&mut h, CMD_SHIFT, Key::K, false)), Some(Chord::cmd_shift(K::Letter('K'))));
    assert_eq!(recorded(&press(&mut h, Modifiers::COMMAND, Key::P, false)), Some(Chord::cmd(K::Letter('P'))), "not the palette");
    let esc = press(&mut h, Modifiers::NONE, Key::Escape, false);
    assert!(matches!(esc.as_slice(), [Msg::Keys(KeysMsg::CancelRecord)]), "Esc cancels and doesn't close Preferences");
    assert_eq!(recorded(&press(&mut h, Modifiers::NONE, Key::F5, false)), Some(Chord::plain(K::F(5))));
    // Still held down (no release came): egui marks the second press a repeat.
    assert!(press(&mut h, Modifiers::NONE, Key::F5, true).is_empty(), "a repeat is ignored");
    assert!(press(&mut h, Modifiers::NONE, Key::F13, false).is_empty(), "a key Glowfly can't bind is swallowed");
    assert!(h.state().st.prefs.as_ref().unwrap().keys.recording.is_some(), "update ends the recording, not the view");
}

#[test]
fn keys_work_normally_on_other_pages() {
    let mut h = harness(PrefPage::General);
    let msgs = press(&mut h, Modifiers::NONE, Key::F5, false);
    assert!(matches!(msgs.as_slice(), [Msg::Repo(crate::msg::RepoMsg::Refresh)]));
    let msgs = press(&mut h, Modifiers::NONE, Key::Escape, false);
    assert!(matches!(msgs.as_slice(), [Msg::Prefs(crate::msg::PrefsMsg::ClosePrefs)]));
}

#[test]
fn presses_become_chords_on_each_system() {
    let pc_ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    let mods = |command, ctrl| Mods { command, ctrl, shift: false, alt: false };
    assert_eq!(chord_from_press(Key::K, pc_ctrl, false).map(|c| c.mods), Some(mods(true, false)), "Ctrl off macOS is Mod");
    let mac_cmd = Modifiers { mac_cmd: true, command: true, ..Modifiers::NONE };
    assert_eq!(chord_from_press(Key::K, mac_cmd, true).map(|c| c.mods), Some(mods(true, false)), "Cmd is Mod");
    assert_eq!(chord_from_press(Key::K, Modifiers::CTRL, true).map(|c| c.mods), Some(mods(false, true)), "Ctrl on macOS is Ctrl");
    let both = Modifiers { ctrl: true, ..mac_cmd };
    assert_eq!(chord_from_press(Key::K, both, true).map(|c| c.mods), Some(mods(true, true)));
    let alt_shift = Modifiers { alt: true, shift: true, ..Modifiers::NONE };
    assert_eq!(
        chord_from_press(Key::F7, alt_shift, false),
        Some(Chord { mods: Mods { alt: true, shift: true, ..Mods::default() }, key: K::F(7) })
    );
    for key in [Key::F13, Key::Copy, Key::Pipe, Key::OpenCurlyBracket, Key::Colon, Key::Questionmark] {
        assert_eq!(chord_from_press(key, Modifiers::NONE, false), None, "{key:?}");
    }
}

#[test]
fn egui_keys_and_key_names_agree() {
    for &key in Key::ALL {
        if let Some(name) = key_name(key) {
            assert_eq!(egui_key(name), Some(key), "{key:?}");
        }
    }
    assert_eq!(key_name(Key::ArrowLeft), Some(K::Left));
    assert_eq!(key_name(Key::Num0), Some(K::Digit(0)));
    assert_eq!(egui_key(K::DigitRange(1, 8)), None);
}
