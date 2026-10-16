//! Tooltips and texts that name a key read it from the effective keymap: a rebound key shows
//! its new key, a removed one leaves no parentheses.

use crate::keymap::{BindingId as I, Chord, KeyName as K, Keymap};

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
    assert!(super::toolbar::terminal_tip(&keys, false).starts_with("Show the terminal panel (F8)\n"));
    keys.set(I::ToggleTerminal, Vec::new());
    let tip = super::toolbar::terminal_tip(&keys, true);
    assert!(tip.starts_with("Hide the terminal panel\n") && !tip.contains('('), "{tip}");
}

#[test]
fn discard_all_names_undos_current_key() {
    let mut keys = Keymap::default();
    keys.set(I::Undo, f8());
    assert!(super::details::discard_all_text(&keys).contains("Undo (F8) or applying"));
    keys.set(I::Undo, Vec::new());
    let text = super::details::discard_all_text(&keys);
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
    assert_eq!(super::commit_box::shortcuts_tip(&keys), want);
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
