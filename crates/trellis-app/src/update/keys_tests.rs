//! Headless tests for Preferences > Keyboard: recording a key, refusals, clashes (Swap, Use
//! here, fixed bindings), removing and resetting keys, Reset all, loading bad settings at
//! startup, and tooltips following the change.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::keymap::{BindingId as I, Chord, KeyName as K, Keymap, Slot};
use crate::msg::{KeysMsg, Msg};
use crate::state::keys_page::{KeysPage, Pending};
use crate::state::{AppState, PrefPage, PrefsState};
use std::collections::BTreeMap;
use std::time::Instant;
use trellis_core::settings::Settings;

fn page() -> AppState {
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Keyboard));
    st
}

fn send(st: &mut AppState, m: KeysMsg) -> Vec<Effect> {
    update(st, Msg::Keys(m), Instant::now())
}

fn keys(st: &AppState) -> &KeysPage {
    &st.prefs.as_ref().unwrap().keys
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

/// Records `c` for `id`'s `slot`, returning the effects of the recorded key.
fn record(st: &mut AppState, id: crate::keymap::BindingId, slot: Slot, c: Chord) -> Vec<Effect> {
    assert!(send(st, KeysMsg::Record { id, slot }).is_empty());
    send(st, KeysMsg::Recorded(c))
}

fn shift_n() -> Chord {
    Chord::cmd_shift(K::Letter('N'))
}

fn mod_t() -> Chord {
    Chord::cmd(K::Letter('T'))
}

#[test]
fn record_and_apply() {
    let mut st = page();
    send(&mut st, KeysMsg::Record { id: I::NewTab, slot: Slot::Replace(0) });
    assert!(keys(&st).recording.is_some());
    let fx = send(&mut st, KeysMsg::Recorded(shift_n()));
    assert_eq!(st.keymap.chords(I::NewTab), [shift_n()]);
    let want: BTreeMap<String, Vec<String>> = [("new-tab".to_owned(), vec!["Mod+Shift+N".to_owned()])].into();
    assert_eq!(st.settings.keybindings, want);
    assert_eq!(saves(&fx), 1);
    assert!(keys(&st).recording.is_none());
}

#[test]
fn add_keeps_existing() {
    let mut st = page();
    record(&mut st, I::NewTab, Slot::Add, Chord::plain(K::F(9)));
    assert_eq!(st.keymap.chords(I::NewTab), [mod_t(), Chord::plain(K::F(9))]);
}

#[test]
fn recorded_without_recording_is_ignored() {
    let mut st = page();
    assert!(send(&mut st, KeysMsg::Recorded(shift_n())).is_empty());
    assert_eq!(st.keymap, Keymap::default());
    // Nor outside the Keyboard page.
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    send(&mut st, KeysMsg::Record { id: I::NewTab, slot: Slot::Add });
    assert!(send(&mut st, KeysMsg::Recorded(shift_n())).is_empty());
    assert_eq!(st.keymap, Keymap::default());
}

#[test]
fn record_fixed_is_ignored() {
    let mut st = page();
    send(&mut st, KeysMsg::Record { id: I::CloseView, slot: Slot::Replace(0) });
    assert!(keys(&st).recording.is_none());
}

#[test]
fn same_chord_changes_nothing() {
    let mut st = page();
    let fx = record(&mut st, I::NewTab, Slot::Replace(0), mod_t());
    assert_eq!(saves(&fx), 0);
    assert!(keys(&st).pending.is_none() && keys(&st).recording.is_none());
}

#[test]
fn refused_sets_pending() {
    for (id, c) in
        [(I::Refresh, Chord::plain(K::Enter)), (I::Refresh, Chord::cmd(K::Letter('C'))), (I::Palette, Chord::plain(K::Letter('P')))]
    {
        let mut st = page();
        let fx = record(&mut st, id, Slot::Add, c);
        assert_eq!(saves(&fx), 0, "{c:?}");
        assert_eq!(st.keymap, Keymap::default(), "{c:?}");
        assert!(matches!(&keys(&st).pending, Some(Pending::Refused { id: i, .. }) if *i == id), "{c:?}");
    }
    let mut st = page();
    record(&mut st, I::Undo, Slot::Replace(0), Chord::plain(K::Letter('Z')));
    assert_eq!(st.keymap.chords(I::Undo), [Chord::plain(K::Letter('Z'))], "Undo works only outside text fields");
}

#[test]
fn clash_swap() {
    let mut st = page();
    let fx = record(&mut st, I::Refresh, Slot::Replace(0), mod_t());
    assert_eq!(saves(&fx), 0);
    let f5 = Chord::plain(K::F(5));
    let want = Pending::Clash { id: I::Refresh, slot: Slot::Replace(0), chord: mod_t(), other: I::NewTab, fixed: false, swap: Some(f5) };
    assert_eq!(keys(&st).pending, Some(want));
    let fx = send(&mut st, KeysMsg::Swap);
    assert_eq!(st.keymap.chords(I::Refresh), [mod_t()]);
    assert_eq!(st.keymap.chords(I::NewTab), [f5]);
    assert_eq!(st.settings.keybindings.keys().collect::<Vec<_>>(), ["new-tab", "refresh"]);
    assert_eq!(saves(&fx), 1);
    assert!(keys(&st).pending.is_none());
}

#[test]
fn clash_use_here() {
    let mut st = page();
    record(&mut st, I::Refresh, Slot::Replace(0), mod_t());
    let fx = send(&mut st, KeysMsg::UseHere);
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.keymap.chords(I::Refresh), [mod_t()]);
    assert!(st.keymap.chords(I::NewTab).is_empty());
    assert_eq!(st.keymap.tip("New tab", I::NewTab), "New tab");
    assert_eq!(st.settings.keybindings.get("new-tab"), Some(&Vec::new()));
}

#[test]
fn clash_fixed_offers_no_swap() {
    let mut st = page();
    record(&mut st, I::Refresh, Slot::Replace(0), Chord::cmd(K::Digit(3)));
    assert!(matches!(keys(&st).pending, Some(Pending::Clash { other: I::GoToTab, fixed: true, swap: None, .. })));
    assert!(send(&mut st, KeysMsg::Swap).is_empty());
    assert!(send(&mut st, KeysMsg::UseHere).is_empty());
    assert_eq!(st.keymap, Keymap::default());
    send(&mut st, KeysMsg::DismissPending);
    assert!(keys(&st).pending.is_none());
}

#[test]
fn remove_chord() {
    let mut st = page();
    let fx = send(&mut st, KeysMsg::Remove { id: I::Palette, index: 1 });
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.keymap.chords(I::Palette), [Chord::cmd(K::Letter('P'))]);
    let fx = send(&mut st, KeysMsg::Remove { id: I::Palette, index: 0 });
    assert_eq!(saves(&fx), 1);
    assert!(st.keymap.chords(I::Palette).is_empty());
    assert_eq!(st.settings.keybindings.get("palette"), Some(&Vec::new()));
    assert!(send(&mut st, KeysMsg::Remove { id: I::Palette, index: 0 }).is_empty(), "nothing left to remove");
}

#[test]
fn reset_takes_default_back() {
    let mut st = page();
    record(&mut st, I::Refresh, Slot::Replace(0), mod_t());
    send(&mut st, KeysMsg::UseHere);
    let fx = send(&mut st, KeysMsg::Reset(I::NewTab));
    assert_eq!(saves(&fx), 1);
    assert_eq!(st.keymap.chords(I::NewTab), [mod_t()]);
    assert!(st.keymap.chords(I::Refresh).is_empty());
    match &keys(&st).pending {
        Some(Pending::Notice { id: I::NewTab, text }) => assert!(text.contains("taken from Refresh"), "{text}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn reset_all() {
    let mut st = page();
    send(&mut st, KeysMsg::AskResetAll);
    assert!(!keys(&st).confirm_reset_all, "nothing to reset");
    record(&mut st, I::NewTab, Slot::Replace(0), shift_n());
    send(&mut st, KeysMsg::AskResetAll);
    assert!(keys(&st).confirm_reset_all);
    send(&mut st, KeysMsg::CancelResetAll);
    assert!(!keys(&st).confirm_reset_all);
    send(&mut st, KeysMsg::AskResetAll);
    let fx = send(&mut st, KeysMsg::ResetAll);
    assert_eq!(st.keymap, Keymap::default());
    assert!(st.settings.keybindings.is_empty());
    assert_eq!(saves(&fx), 1);
    assert!(!keys(&st).confirm_reset_all);
}

#[test]
fn startup_ignores_bad_entries() {
    let mut s = Settings::default();
    for (k, v) in [("bogus", "F9"), ("close-view", "F9"), ("palette", "P"), ("refresh", "Mod+R")] {
        s.keybindings.insert(k.to_owned(), vec![v.to_owned()]);
    }
    let st = AppState::new(s, Vec::new(), Instant::now());
    let lines: Vec<&str> = st.log.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines.iter().all(|l| l.starts_with("Ignored keyboard setting ")), "{lines:?}");
    assert!(!st.log.iter().any(|l| l.ok));
    assert_eq!(st.keymap.chords(I::Palette), Keymap::default().chords(I::Palette), "a refused entry leaves the action as it was");
    assert_eq!(st.keymap.chords(I::Refresh), [Chord::cmd(K::Letter('R'))]);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn tooltips_follow_rebinding() {
    // The Undo tooltip draws the binding's first chord as key caps (ui/toolbar_tips.rs).
    let mut st = page();
    let shift_k = Chord::cmd_shift(K::Letter('K'));
    record(&mut st, I::Undo, Slot::Replace(0), shift_k);
    assert_eq!(st.keymap.chords(I::Undo).first(), Some(&shift_k));
    assert_eq!(st.keymap.hint(I::Undo), Some("Ctrl+Shift+K"));
    st.keymap.set(I::Undo, Vec::new());
    assert!(st.keymap.chords(I::Undo).is_empty(), "no caps to draw");
}

#[test]
fn watch_notices_name_refreshs_current_key() {
    let mut keys = Keymap::default();
    keys.set(I::Refresh, vec![Chord::plain(K::F(9))]);
    assert!(super::watch::failed_text(&keys).ends_with("or you press F9."));
    let remote = super::watch::remote_text(std::path::Path::new("share"), "SMB", &keys);
    assert!(remote.ends_with("when you press F9."), "{remote}");
    keys.set(I::Refresh, Vec::new());
    assert!(super::watch::failed_text(&keys).ends_with("or you choose Refresh."));
}
