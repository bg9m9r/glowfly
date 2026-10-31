//! Menu mnemonics: the letter each title gets in English, German and Spanish, the letters the
//! keymap's Alt keys reserve, and the keyboard state's steps.

use super::*;
use crate::custom_commands::CommandRef;
use crate::keymap::Chord;
use glowfly_core::custom_commands::Source;

const NONE: &[char] = &[];

/// (index, letter) of each title's mnemonic.
fn marks(titles: [&str; 5], reserved: &[char]) -> Vec<Option<(usize, char)>> {
    mnemonics(titles, reserved).iter().map(|m| m.map(|m| (m.index, m.letter))).collect()
}

#[test]
fn english_takes_each_first_letter() {
    let want: Vec<_> = ['f', 'e', 'v', 'w', 'h'].map(|c| Some((0, c))).into();
    assert_eq!(marks(["File", "Edit", "View", "Window", "Help"], NONE), want);
}

#[test]
fn german_takes_each_first_letter() {
    let want: Vec<_> = ['d', 'b', 'a', 'f', 'h'].map(|c| Some((0, c))).into();
    assert_eq!(marks(["Datei", "Bearbeiten", "Ansicht", "Fenster", "Hilfe"], NONE), want);
}

#[test]
fn spanish_moves_on_past_taken_letters() {
    let got = marks(["Archivo", "Editar", "Ver", "Ventana", "Ayuda"], NONE);
    assert_eq!(got, [Some((0, 'a')), Some((0, 'e')), Some((0, 'v')), Some((2, 'n')), Some((1, 'y'))]);
}

#[test]
fn reserved_letters_are_skipped() {
    let got = marks(["File", "Edit", "View", "Window", "Help"], &['f']);
    assert_eq!(got[0], Some((1, 'i')));
    // Edit's E, then d (i is File's now).
    let got = marks(["File", "Edit", "View", "Window", "Help"], &['f', 'e']);
    assert_eq!(got[..2], [Some((1, 'i')), Some((1, 'd'))]);
}

#[test]
fn letters_collide_whatever_their_case() {
    let got = marks(["abc", "ABC", "Bx", "cab", "Q"], NONE);
    assert_eq!(got, [Some((0, 'a')), Some((1, 'b')), Some((1, 'x')), Some((0, 'c')), Some((0, 'q'))]);
}

#[test]
fn no_free_ascii_letter_means_no_mnemonic() {
    let got = marks(["ab", "ba", "Ää ü", "1 2", "Ñu"], NONE);
    assert_eq!(got, [Some((0, 'a')), Some((0, 'b')), None, None, Some((1, 'u'))], "non-ASCII letters are never chosen");
}

#[test]
fn the_default_alt_keys_reserve_c_k_and_s() {
    assert_eq!(reserved(&Keymap::default(), &[]), ['c', 'k', 's']);
    let key = |chord: Chord| -> CommandKey { (chord, CommandRef { source: Source::User, name: "Deploy".into() }, "Deploy".into()) };
    let custom = key(Chord::alt(KeyName::Letter('G')));
    // Alt+Shift+F or Ctrl+Alt+E aren't plain Alt keys.
    let shifted = key(Chord { mods: Mods { alt: true, shift: true, ..Default::default() }, key: KeyName::Letter('F') });
    let cmd_alt = key(Chord::cmd_alt(KeyName::Letter('E')));
    assert_eq!(reserved(&Keymap::default(), &[custom, shifted, cmd_alt]), ['c', 'g', 'k', 's']);
}

#[test]
fn menus_are_found_by_letter() {
    let m = mnemonics(["File", "Edit", "View", "Window", "Help"], NONE);
    assert_eq!(menu_for('w', &m), Some(TopMenu::Window));
    assert_eq!(menu_for('x', &m), None);
}

#[test]
fn stepping_wraps_both_ways() {
    assert_eq!(TopMenu::Help.step(1), TopMenu::File);
    assert_eq!(TopMenu::File.step(-1), TopMenu::Help);
    assert_eq!(TopMenu::Edit.step(1), TopMenu::View);
    assert_eq!(TopMenu::View.step(-2), TopMenu::File);
    assert_eq!(TopMenu::ALL.map(|m| m.title(glowfly_core::i18n::Lang::En)), ["File", "Edit", "View", "Window", "Help"]);
}

#[test]
fn the_keyboard_state_follows_the_menu() {
    let mut k = MenuKeys::default();
    k.opened_by_key(TopMenu::Edit);
    assert_eq!(k, MenuKeys { keyboard: true, focus_first: true, only: Some(TopMenu::Edit), ..Default::default() });
    k.focus_first = false;
    k.stepped(1);
    assert_eq!((k.only, k.focus_first), (Some(TopMenu::View), true));
    k.stepped(-2);
    assert_eq!(k.only, Some(TopMenu::File));
    k.back_to = Some(TopMenu::File);
    k.closed();
    assert_eq!(k, MenuKeys::default());
}
