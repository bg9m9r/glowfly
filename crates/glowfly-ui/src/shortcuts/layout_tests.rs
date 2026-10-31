//! Matching the zoom, full-screen and panel keys: zoom and full screen work over everything
//! (typing, the palette, dialogs, the sheet, Preferences, a credential prompt) except a key
//! being recorded; the panel toggles work only with nothing in the way.

use crate::keymap::{BindingId, Chord, KeyName, Slot};
use crate::msg::{KeysMsg, LayoutMsg, Msg};
use crate::state::askpass::PendingPrompt;
use crate::state::keys_page::Recording;
use crate::state::layout::Panel;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::askpass::Prompt;
use glowfly_core::settings::Settings;
use std::time::Instant;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// Draw a text field, and give it focus on the next frame.
    field: Option<(String, bool)>,
}

type H = Harness<'static, Keys>;

fn state() -> AppState {
    AppState::new(Settings::default(), Vec::new(), Instant::now())
}

fn harness(st: AppState, field: bool) -> H {
    let keys = Keys { st, msgs: Vec::new(), field: field.then(|| (String::new(), true)) };
    let mut h = crate::ui::snapshot_check::builder().build_state(
        |ctx, k: &mut Keys| {
            super::input(ctx, &mut k.st, &mut k.msgs);
            if let Some((text, focus)) = k.field.as_mut() {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let r = ui.text_edit_singleline(text);
                    if std::mem::take(focus) {
                        r.request_focus();
                    }
                });
            }
        },
        keys,
    );
    h.run();
    h
}

/// Presses `key` with `modifiers` for one frame and returns the messages it produced.
fn press(h: &mut H, modifiers: Modifiers, key: Key) -> Vec<Msg> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs)
}

fn layout(msgs: &[Msg]) -> Option<LayoutMsg> {
    match msgs {
        [Msg::Layout(m)] => Some(m.clone()),
        _ => None,
    }
}

const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };

/// The three zoom keys each give their message in `h`.
fn zooms(h: &mut H, what: &str) {
    assert_eq!(layout(&press(h, Modifiers::COMMAND, Key::Equals)), Some(LayoutMsg::ZoomIn), "Ctrl+= {what}");
    assert_eq!(layout(&press(h, Modifiers::COMMAND, Key::Minus)), Some(LayoutMsg::ZoomOut), "Ctrl+- {what}");
    assert_eq!(layout(&press(h, Modifiers::COMMAND, Key::Num0)), Some(LayoutMsg::ZoomReset), "Ctrl+0 {what}");
}

/// The default full-screen press on this system: Ctrl+Cmd+F on macOS, F11 elsewhere.
fn full_screen_press() -> (Modifiers, Key) {
    if cfg!(target_os = "macos") {
        (Modifiers { ctrl: true, mac_cmd: true, command: true, ..Default::default() }, Key::F)
    } else {
        (Modifiers::NONE, Key::F11)
    }
}

/// The full-screen key gives its message in `h`.
fn full_screen(h: &mut H, what: &str) {
    let (m, key) = full_screen_press();
    assert_eq!(layout(&press(h, m, key)), Some(LayoutMsg::ToggleFullscreen), "{what}");
}

#[test]
fn zoom_keys() {
    let mut h = harness(state(), false);
    zooms(&mut h, "with nothing open");
    assert_eq!(layout(&press(&mut h, Modifiers::COMMAND, Key::Plus)), Some(LayoutMsg::ZoomIn), "numpad +");
    // Ctrl+Shift+= on a US layout arrives as Plus with Shift: the chord without Shift answers.
    assert_eq!(layout(&press(&mut h, CMD_SHIFT, Key::Plus)), Some(LayoutMsg::ZoomIn));
    assert_eq!(layout(&press(&mut h, CMD_SHIFT, Key::Equals)), Some(LayoutMsg::ZoomIn));
}

#[test]
fn zoom_works_over_everything() {
    let mut h = harness(state(), true);
    assert!(h.ctx.wants_keyboard_input(), "the field has focus");
    zooms(&mut h, "while typing");

    let mut st = state();
    st.palette = Some(Default::default());
    zooms(&mut harness(st, false), "with the palette open");
    let mut st = state();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    zooms(&mut harness(st, false), "with the sheet open");
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    zooms(&mut harness(st, false), "under Preferences");
    let mut st = state();
    let prompt = PendingPrompt { id: 1, prompt: Prompt::new("Password: ", None), retry: false, rejected_before: false };
    st.askpass.queue.push_back(prompt);
    zooms(&mut harness(st, false), "under a credential prompt");
}

#[test]
fn full_screen_key_works_over_everything() {
    full_screen(&mut harness(state(), false), "with nothing open");
    let mut h = harness(state(), true);
    assert!(h.ctx.wants_keyboard_input(), "the field has focus");
    full_screen(&mut h, "while typing");

    let mut st = state();
    st.palette = Some(Default::default());
    full_screen(&mut harness(st, false), "with the palette open");
    let mut st = state();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    full_screen(&mut harness(st, false), "with the sheet open");
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    full_screen(&mut harness(st, false), "under Preferences");
    let mut st = state();
    let prompt = PendingPrompt { id: 1, prompt: Prompt::new("Password: ", None), retry: false, rejected_before: false };
    st.askpass.queue.push_back(prompt);
    full_screen(&mut harness(st, false), "under a credential prompt");
}

#[test]
fn mac_cmd_ctrl_f_is_not_search() {
    let mut st = state();
    st.keymap.set(BindingId::ToggleFullscreen, vec![Chord::cmd_ctrl(KeyName::Letter('F'))]);
    let mut h = harness(st, false);
    let cmd_ctrl = Modifiers { ctrl: true, mac_cmd: true, command: true, ..Default::default() };
    assert_eq!(layout(&press(&mut h, cmd_ctrl, Key::F)), Some(LayoutMsg::ToggleFullscreen), "Ctrl+Cmd+F");
    assert!(!h.state().st.view.show_search, "full screen, not search");
    // Search's toggle is view state, not a message.
    assert!(press(&mut h, Modifiers::MAC_CMD | Modifiers::COMMAND, Key::F).is_empty(), "Cmd+F");
    assert!(h.state().st.view.show_search, "Cmd+F shows the search bar");
}

#[test]
fn panel_keys_only_with_nothing_in_the_way() {
    let mut h = harness(state(), false);
    let toggles = [(Key::L, Panel::Left), (Key::D, Panel::Details), (Key::B, Panel::Toolbar)];
    for (key, panel) in toggles {
        assert_eq!(layout(&press(&mut h, CMD_SHIFT, key)), Some(LayoutMsg::TogglePanel(panel)), "{key:?}");
    }
    let mut h = harness(state(), true);
    assert_eq!(layout(&press(&mut h, CMD_SHIFT, Key::L)), Some(LayoutMsg::TogglePanel(Panel::Left)), "while typing");

    let mut st = state();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    assert!(press(&mut harness(st, false), CMD_SHIFT, Key::L).is_empty(), "under the sheet");
    let mut st = state();
    st.palette = Some(Default::default());
    assert!(press(&mut harness(st, false), CMD_SHIFT, Key::L).is_empty(), "under the palette");
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    assert!(press(&mut harness(st, false), CMD_SHIFT, Key::B).is_empty(), "under Preferences");
}

#[test]
fn recording_takes_the_zoom_keys() {
    let mut st = state();
    let mut prefs = PrefsState::new(PrefPage::Keyboard);
    prefs.keys.recording = Some(Recording { id: BindingId::NewTab, slot: Slot::Replace(0) });
    st.prefs = Some(prefs);
    let mut h = harness(st, false);
    let msgs = press(&mut h, Modifiers::COMMAND, Key::Equals);
    assert!(
        matches!(msgs.as_slice(), [Msg::Keys(KeysMsg::Recorded(c))] if c.key == KeyName::Equals && c.mods.command),
        "recorded, not zoomed"
    );
    let (m, key) = full_screen_press();
    let msgs = press(&mut h, m, key);
    assert!(matches!(msgs.as_slice(), [Msg::Keys(KeysMsg::Recorded(_))]), "recorded, not full screen");
}
