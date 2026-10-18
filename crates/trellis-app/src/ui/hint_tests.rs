//! Tooltips and texts that name a key read it from the effective keymap: a rebound key shows
//! its new key, a removed one leaves no parentheses.

use crate::keymap::{BindingId as I, Chord, KeyName as K, Keymap};
use trellis_core::i18n::Lang;

fn f8() -> Vec<Chord> {
    vec![Chord::plain(K::F(8))]
}

#[test]
fn conflict_steps_name_the_current_key() {
    let mut keys = Keymap::default();
    keys.set(I::NextChange, f8());
    keys.set(I::PrevChange, Vec::new());
    assert_eq!(super::conflict::step_tip(&keys, true), "Next conflict (F8)");
    assert_eq!(super::conflict::step_tip(&keys, false), "Previous conflict");
}

#[test]
fn terminal_button_names_the_current_key() {
    let mut keys = Keymap::default();
    keys.set(I::ToggleTerminal, f8());
    let en = trellis_core::i18n::Lang::En;
    assert!(super::toolbar::terminal_tip(en, &keys, false, false).starts_with("Show the terminal panel (F8)\n"));
    keys.set(I::ToggleTerminal, Vec::new());
    let tip = super::toolbar::terminal_tip(en, &keys, true, false);
    assert!(tip.starts_with("Hide the terminal panel\n") && !tip.contains('('), "{tip}");
    assert!(tip.ends_with("\nRight-click to open an external terminal"), "{tip}");
    let tip = super::toolbar::terminal_tip(en, &keys, false, true);
    assert!(tip.ends_with("\nRight-click for a new terminal tab"), "{tip}");
}

#[test]
fn discard_all_names_undos_current_key() {
    let mut keys = Keymap::default();
    keys.set(I::Undo, f8());
    assert!(super::details_wip::discard_all_text(Lang::En, &keys).contains("Undo (F8) or applying"));
    let german = super::details_wip::discard_all_text(Lang::De, &keys);
    assert!(german.contains("(F8)") && german.contains(trellis_core::undo::DISCARD_STASH_MESSAGE), "{german}");
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
    assert_eq!(super::commit_box::shortcuts_tip(trellis_core::i18n::Lang::En, &keys), want);
}

#[test]
fn menu_bar_items_name_the_current_key() {
    let mut keys = Keymap::default();
    assert_eq!(super::menu_bar::hint(&keys, Some(I::Undo)), keys.hint(I::Undo).unwrap());
    keys.set(I::Undo, f8());
    assert_eq!(super::menu_bar::hint(&keys, Some(I::Undo)), "F8");
    keys.set(I::Undo, Vec::new());
    assert_eq!(super::menu_bar::hint(&keys, Some(I::Undo)), "");
    assert_eq!(super::menu_bar::hint(&keys, None), "");
}
