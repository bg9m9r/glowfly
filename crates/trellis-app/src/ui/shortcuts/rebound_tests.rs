//! Matching through the effective keymap: rebound and removed keys, the exact chord beating the
//! Shift-stripped one, and the keys that used to be matched by hand (F5, Ctrl+comma, Ctrl+F,
//! Ctrl+P, F7 and Alt+arrows, Ctrl+1 to 9) firing only where they did.

use crate::keymap::{BindingId as I, Chord, KeyAction, KeyName as K};
use crate::msg::{ConflictMsg, DiffMsg, Msg, PaletteMsg, PrefsMsg, RepoMsg, SearchMsg, TabsMsg};
use crate::state::blame::BlameView;
use crate::state::conflict::ConflictView;
use crate::state::{AppState, Modal, PrefPage, PrefsState, Tab};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use std::time::Instant;
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::Settings;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// Draw a text field and keep it focused.
    field: Option<String>,
}

type H = Harness<'static, Keys>;

fn state() -> AppState {
    AppState::new(Settings::default(), Vec::new(), Instant::now())
}

fn with_diff(mut st: AppState) -> AppState {
    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((DiffTarget::Unstaged("f.txt".into()), empty.into()));
    st
}

fn harness(st: AppState, field: bool) -> H {
    let keys = Keys { st, msgs: Vec::new(), field: field.then(String::new) };
    let mut h = Harness::new_state(
        |ctx, k: &mut Keys| {
            super::input(ctx, &mut k.st, &mut k.msgs);
            if let Some(text) = k.field.as_mut() {
                egui::CentralPanel::default().show(ctx, |ui| ui.text_edit_singleline(text).request_focus());
            }
        },
        keys,
    );
    h.run();
    h
}

/// What one press does, named.
fn press(h: &mut H, modifiers: Modifiers, key: Key) -> Vec<String> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = modifiers;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs).iter().map(name).collect()
}

fn name(m: &Msg) -> String {
    match m {
        Msg::Repo(RepoMsg::Refresh) => "refresh".into(),
        Msg::Tabs(TabsMsg::NewTab) => "new tab".into(),
        Msg::Tabs(TabsMsg::SelectTab(n)) => format!("tab {n}"),
        Msg::Shortcut(a) => format!("{a:?}"),
        Msg::Prefs(PrefsMsg::OpenPrefs(_)) => "prefs".into(),
        Msg::Palette(PaletteMsg::TogglePalette) => "palette".into(),
        Msg::Search(SearchMsg::ToggleFilter) => "filter".into(),
        Msg::Diff(DiffMsg::JumpToChange(d)) => format!("jump {d}"),
        Msg::Conflict(ConflictMsg::ConflictStep(d)) => format!("conflict {d}"),
        Msg::Job(crate::msg::JobMsg::Undo) => "undo".into(),
        _ => "other".into(),
    }
}

const NONE: Modifiers = Modifiers::NONE;
const CMD: Modifiers = Modifiers::COMMAND;
const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };

fn quiet() -> Vec<String> {
    Vec::new()
}

#[test]
fn rebound_and_removed_keys() {
    let mut st = state();
    st.keymap.set(I::Refresh, vec![Chord::cmd(K::Letter('R'))]);
    st.keymap.set(I::NewTab, Vec::new());
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, CMD, Key::R), ["refresh"]);
    assert_eq!(press(&mut h, NONE, Key::F5), quiet());
    assert_eq!(press(&mut h, CMD, Key::T), quiet());
}

#[test]
fn shift_fallback_respects_owned_chords() {
    let mut st = state();
    st.keymap.set(I::Refresh, vec![Chord::cmd(K::Letter('K'))]);
    st.keymap.set(I::Undo, vec![Chord::cmd_shift(K::Letter('K'))]);
    let mut h = harness(st, true);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::K), quiet(), "Undo owns it and is off while typing");
    assert_eq!(press(&mut h, CMD, Key::K), ["refresh"]);
}

#[test]
fn exact_beats_shift_stripped() {
    let mut h = harness(with_diff(state()), false);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::S), ["StageAll"]);
    assert_eq!(press(&mut h, Modifiers::SHIFT, Key::S), ["Stage"], "Shift+S still stages");
    assert_eq!(press(&mut h, Modifiers::SHIFT, Key::F5), ["refresh"]);
}

#[test]
fn refresh_prefs_search_and_palette() {
    let mut h = harness(state(), false);
    assert_eq!(press(&mut h, NONE, Key::F5), ["refresh"]);
    assert_eq!(press(&mut h, CMD, Key::Comma), ["prefs"]);
    assert_eq!(press(&mut h, CMD, Key::P), ["palette"]);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::P), ["palette"]);
    assert_eq!(press(&mut h, CMD, Key::F), quiet());
    assert!(h.state().st.view.show_search, "Ctrl+F shows the bar");
    assert_eq!(press(&mut h, CMD, Key::F), quiet());
    assert!(!h.state().st.view.show_search, "and hides it");
    assert_eq!(press(&mut h, CMD_SHIFT, Key::F), ["filter"]);
    assert!(h.state().st.view.show_search);

    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, NONE, Key::F5), ["refresh"], "F5 works under Preferences");
    assert_eq!(press(&mut h, CMD, Key::Comma), quiet());
    assert_eq!(press(&mut h, CMD, Key::P), quiet());
    assert_eq!(press(&mut h, CMD, Key::F), quiet());

    let mut st = state();
    st.palette = Some(Default::default());
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, CMD, Key::Comma), quiet(), "not under the palette");
    assert_eq!(press(&mut h, CMD, Key::P), ["palette"], "closes it");

    let mut st = state();
    st.modal =
        Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Repo(RepoMsg::Refresh)) });
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, CMD, Key::P), quiet(), "not under a dialog");
}

#[test]
fn stepping_keys() {
    let mut h = harness(with_diff(state()), false);
    assert_eq!(press(&mut h, NONE, Key::F7), ["jump 1"]);
    assert_eq!(press(&mut h, Modifiers::SHIFT, Key::F7), ["jump -1"]);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::ArrowDown), ["jump 1"]);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::ArrowUp), ["jump -1"]);

    let mut st = state();
    st.conflict = Some(ConflictView::loading("f.txt".into()));
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, NONE, Key::F7), ["conflict 1"]);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::ArrowUp), ["conflict -1"]);

    let mut st = with_diff(state());
    st.blame = Some(BlameView::loading("f.txt".into(), None));
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, NONE, Key::F7), quiet(), "not in blame");

    let mut h = harness(with_diff(state()), true);
    assert_eq!(press(&mut h, NONE, Key::F7), quiet(), "not while typing");
    assert_eq!(press(&mut h, Modifiers::ALT, Key::ArrowDown), quiet());

    let mut st = with_diff(state());
    st.keymap.set(I::PrevChange, vec![Chord::cmd_shift(K::Letter('J'))]);
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, CMD_SHIFT, Key::J), ["jump -1"]);
    assert_eq!(press(&mut h, Modifiers::SHIFT, Key::F7), ["jump 1"], "Shift+F7 is F7's now, Shift stripped");
}

#[test]
fn tab_digits() {
    let mut st = state();
    st.tabs = vec![Tab::empty(), Tab::empty(), Tab::empty(), Tab::empty()];
    let mut h = harness(st, false);
    let digits = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8];
    for (n, key) in digits.into_iter().enumerate() {
        assert_eq!(press(&mut h, CMD, key), [format!("tab {n}")], "{key:?}");
    }
    assert_eq!(press(&mut h, CMD, Key::Num9), ["tab 3"], "the last tab");
    assert_eq!(press(&mut h, Modifiers::ALT, Key::Num2), quiet());
}

#[test]
fn staging_keys_still_ask_update() {
    let mut h = harness(with_diff(state()), false);
    assert_eq!(press(&mut h, NONE, Key::S), [format!("{:?}", KeyAction::Stage)]);
}

#[test]
fn favorite_digits_only_while_not_typing() {
    const CMD_ALT: Modifiers = Modifiers { alt: true, ..Modifiers::COMMAND };
    let favorite = |h: &mut H, key| press(h, CMD_ALT, key).len();
    let mut h = harness(state(), false);
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = CMD_ALT;
    input.events.push(Event::Key { key: Key::Num2, physical_key: None, pressed: true, repeat: false, modifiers: CMD_ALT });
    h.step();
    assert!(matches!(&h.state().msgs[..], [Msg::Recent(crate::msg::RecentMsg::OpenFavorite(1))]), "Ctrl+Alt+2 opens favorite 2");
    h.input_mut().modifiers = NONE;
    assert_eq!(press(&mut h, CMD, Key::Num2), ["tab 1"], "Ctrl+2 still goes to a tab");
    // AltGr is Ctrl+Alt on Windows: a focused field keeps the digit for its character.
    let mut h = harness(state(), true);
    assert_eq!(favorite(&mut h, Key::Num2), 0);
    let mut st = state();
    st.palette = Some(Default::default());
    assert_eq!(favorite(&mut harness(st, false), Key::Num3), 0, "not under the palette");
    let mut st = state();
    st.modal = Some(Modal::FindRepos { filter: String::new() });
    assert_eq!(favorite(&mut harness(st, false), Key::Num3), 0, "not under a dialog");
}
