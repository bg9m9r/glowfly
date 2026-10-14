//! Matching the newer keys: which presses become `Msg::Shortcut`, and when nothing does
//! (dialogs, the palette, Preferences, a credential prompt, a focused text field).

use crate::keymap::KeyAction;
use crate::msg::Msg;
use crate::state::askpass::PendingPrompt;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use std::time::Instant;
use trellis_core::askpass::Prompt;
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::Settings;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// Draw a text field, and give it focus on the next frame.
    field: Option<(String, bool)>,
}

type H = Harness<'static, Keys>;

fn diff(target: DiffTarget) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((target, empty.into()));
    st
}

fn unstaged() -> AppState {
    diff(DiffTarget::Unstaged("f.txt".into()))
}

fn harness(st: AppState, field: bool) -> H {
    let keys = Keys { st, msgs: Vec::new(), field: field.then(|| (String::new(), true)) };
    let mut h = Harness::new_state(
        |ctx, k: &mut Keys| {
            // Like the shell: keys before the views draw.
            super::keys(ctx, &k.st, &mut k.msgs);
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

/// Presses `key` with `modifiers` for one frame and returns the actions it produced.
fn press(h: &mut H, modifiers: Modifiers, key: Key) -> Vec<KeyAction> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    let msgs = std::mem::take(&mut h.state_mut().msgs);
    msgs.iter().map(|m| if let Msg::Shortcut(a) = m { *a } else { panic!("unexpected message") }).collect()
}

const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };

#[test]
fn staging_keys() {
    let mut h = harness(unstaged(), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), [KeyAction::Stage]);
    assert_eq!(press(&mut h, Modifiers::SHIFT, Key::S), [KeyAction::Stage], "Shift is tolerated");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), [KeyAction::StageAll], "not Stage as well");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::U), [KeyAction::UnstageAll]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::U), [KeyAction::Unstage], "update decides U does nothing here");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::M), [KeyAction::FocusMessage]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), [KeyAction::ToggleSheet]);

    let mut h = harness(diff(DiffTarget::Staged("f.txt".into())), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::U), [KeyAction::Unstage]);
    let mut h = harness(diff(DiffTarget::Commit(git2::Oid::zero(), "f.txt".into())), false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), [], "not a working-tree diff");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), [KeyAction::StageAll], "works without a working-tree diff");
}

#[test]
fn nothing_under_dialogs_or_the_palette() {
    let mut st = unstaged();
    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Refresh) });
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), []);

    let mut st = unstaged();
    st.palette = Some(Default::default());
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), []);

    let mut st = unstaged();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::M), []);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), [KeyAction::ToggleSheet], "closes the sheet");
}

/// Every newer key, on a working-tree diff with files to stage, gives nothing.
fn all_quiet(st: AppState, what: &str) {
    let mut h = harness(st, false);
    for (mods, key) in [
        (Modifiers::NONE, Key::S),
        (Modifiers::NONE, Key::U),
        (CMD_SHIFT, Key::S),
        (CMD_SHIFT, Key::U),
        (CMD_SHIFT, Key::M),
        (Modifiers::COMMAND, Key::Slash),
    ] {
        assert_eq!(press(&mut h, mods, key), [], "{mods:?} {key:?} under {what}");
    }
}

#[test]
fn nothing_under_preferences_or_a_credential_prompt() {
    let mut st = unstaged();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    all_quiet(st, "Preferences");

    let mut st = unstaged();
    let prompt = PendingPrompt { id: 1, prompt: Prompt::new("Password: ", None), retry: false, rejected_before: false };
    st.askpass.queue.push_back(prompt);
    all_quiet(st, "a credential prompt");

    let st = diff(DiffTarget::Staged("f.txt".into()));
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::U), [KeyAction::Unstage], "U works with nothing in the way");
}

#[test]
fn a_focused_text_field_keeps_plain_keys() {
    let mut h = harness(unstaged(), true);
    assert!(h.ctx.wants_keyboard_input(), "the field has focus");
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), []);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::M), [KeyAction::FocusMessage]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Slash), [KeyAction::ToggleSheet]);
}
