//! Tooltips and texts that name a key read it from the effective keymap: a rebound key shows
//! its new key, a removed one leaves no parentheses.

use crate::keymap::{BindingId as I, Chord, KeyName as K, Keymap};
use glowfly_core::i18n::Lang;

fn f8() -> Vec<Chord> {
    vec![Chord::plain(K::F(8))]
}

#[test]
fn conflict_steps_name_the_current_key() {
    let mut keys = Keymap::default();
    keys.set(I::NextChange, f8());
    keys.set(I::PrevChange, Vec::new());
    assert_eq!(super::conflict::step_tip(&keys, Lang::En, true), "Next conflict (F8)");
    assert_eq!(super::conflict::step_tip(&keys, Lang::En, false), "Previous conflict");
}

#[test]
fn terminal_button_names_the_current_key() {
    use crate::state::tip::TipSub;
    let mut st = crate::state::AppState::new(Default::default(), Vec::new(), std::time::Instant::now());
    st.keymap.set(I::ToggleTerminal, f8());
    let tip = super::toolbar::tips::terminal(&st);
    assert_eq!(tip.title, "Show the terminal panel");
    // The caps are drawn from the keymap's first chord when the tooltip shows.
    assert_eq!(tip.sub, Some(TipSub::Keys(I::ToggleTerminal)));
    assert_eq!(st.keymap.chords(I::ToggleTerminal), &f8()[..]);
    assert_eq!(tip.body.as_deref(), Some("Right-click to open an external terminal."));
    st.settings.terminal.open_in_panel = true;
    assert_eq!(super::toolbar::tips::terminal(&st).body.as_deref(), Some("Right-click for a new terminal tab."));
}

#[test]
fn discard_all_names_undos_current_key() {
    let mut keys = Keymap::default();
    keys.set(I::Undo, f8());
    assert!(super::details_wip::discard_all_text(Lang::En, &keys).contains("Undo (F8) or applying"));
    let german = super::details_wip::discard_all_text(Lang::De, &keys);
    assert!(german.contains("(F8)") && german.contains(glowfly_core::undo::DISCARD_STASH_MESSAGE), "{german}");
    keys.set(I::Undo, Vec::new());
    let text = super::details_wip::discard_all_text(Lang::En, &keys);
    assert!(text.contains(" Undo or applying") && !text.contains('('), "{text}");
}

#[test]
fn commit_tip_reads_the_keymap() {
    let keys = Keymap::default();
    let want = format!(
        "{} commits from the message fields; {} stages all changes first.",
        keys.hint(I::Commit).expect("Commit has a key"),
        keys.hint(I::StageAllAndCommit).expect("it has a key")
    );
    assert_eq!(super::commit_box::shortcuts_tip(glowfly_core::i18n::Lang::En, &keys), want);
}

#[test]
fn menu_bar_items_name_the_current_key() {
    let mut keys = Keymap::default();
    let hint = |keys: &Keymap| super::menu_bar::hint(keys.chords(I::Undo).first().copied());
    assert_eq!(hint(&keys), keys.hint(I::Undo).unwrap());
    keys.set(I::Undo, f8());
    assert_eq!(hint(&keys), "F8");
    keys.set(I::Undo, Vec::new());
    assert_eq!(hint(&keys), "");
    assert_eq!(super::menu_bar::hint(None), "");
}
