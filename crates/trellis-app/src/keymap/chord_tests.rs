//! Chords: the settings-file form round-trips, parsing is lenient about case, spaces and
//! modifier order but strict about what a chord is, and every key has display text.

use super::*;

#[test]
fn defaults_round_trip_through_the_file_form() {
    for b in BINDINGS {
        for c in b.defaults.iter().filter(|c| !matches!(c.key, KeyName::DigitRange(..))) {
            assert_eq!(Chord::parse(&c.config()), Some(*c), "{:?} as {}", b.id, c.config());
        }
    }
}

#[test]
fn every_key_round_trips_and_has_text() {
    use KeyName as K;
    let mut all = vec![
        K::Enter,
        K::Escape,
        K::Tab,
        K::Backspace,
        K::Space,
        K::Up,
        K::Down,
        K::Left,
        K::Right,
        K::Home,
        K::End,
        K::PageUp,
        K::PageDown,
        K::Insert,
        K::Delete,
        K::Comma,
        K::Period,
        K::Slash,
        K::Backslash,
        K::Semicolon,
        K::Quote,
        K::Backtick,
        K::Minus,
        K::Equals,
        K::Plus,
        K::OpenBracket,
        K::CloseBracket,
    ];
    all.extend(('A'..='Z').map(K::Letter));
    all.extend((0..=9).map(K::Digit));
    all.extend((1..=12).map(K::F));
    for k in all {
        let c = Chord::cmd(k);
        assert_eq!(Chord::parse(&c.config()), Some(c), "{k:?}");
        assert!(!c.parts().last().unwrap().is_empty(), "{k:?} has display text");
        assert!(c.text().is_ascii(), "{k:?} shows as ASCII");
    }
    assert_eq!(Chord::plain(K::PageUp).text(), "PgUp");
    assert_eq!(Chord::plain(K::Backslash).text(), "\\");
    assert_eq!(Chord::plain(K::Backslash).config(), "Backslash");
}

#[test]
fn plus_is_a_named_key() {
    assert_eq!(KeyName::parse("Plus"), Some(KeyName::Plus));
    let c = Chord::parse("Mod+Plus").unwrap();
    assert_eq!(c, Chord::cmd(KeyName::Plus));
    assert_eq!(c.config(), "Mod+Plus");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn plus_reads_as_a_word() {
    assert_eq!(Chord::cmd(KeyName::Plus).text(), "Ctrl+Plus");
}

#[test]
fn parse_is_lenient_about_spelling() {
    let want = Chord::cmd_shift(KeyName::Letter('S'));
    for text in ["Mod+Shift+S", "shift+mod+s", " Shift + MOD + s ", "Cmd+Shift+S"] {
        assert_eq!(Chord::parse(text), Some(want), "{text}");
    }
    assert_eq!(Chord::parse("Ctrl+Tab"), Some(Chord::ctrl(KeyName::Tab)));
    assert_eq!(Chord::parse("alt+down"), Some(Chord::alt(KeyName::Down)));
    assert_eq!(Chord::parse("f12"), Some(Chord::plain(KeyName::F(12))));
    assert_eq!(Chord::parse("Esc"), Some(Chord::plain(KeyName::Escape)));
}

#[test]
fn parse_rejects_what_is_not_one_chord() {
    for text in ["", "Mod+", "Foo", "Mod+Mod+S", "Shift", "S+T", "1-8", "F13", "F0", "+"] {
        assert_eq!(Chord::parse(text), None, "{text:?}");
    }
}

#[test]
fn file_form_orders_modifiers() {
    let c = Chord { mods: Mods { command: true, ctrl: true, shift: true, alt: true }, key: KeyName::Letter('K') };
    assert_eq!(c.config(), "Mod+Ctrl+Shift+Alt+K");
    assert_eq!(Chord::parse("Alt+Shift+Ctrl+Mod+K"), Some(c));
    assert_eq!(Chord::ctrl_shift(KeyName::Tab).config(), "Ctrl+Shift+Tab");
    assert_eq!(Chord::plain(KeyName::F(5)).config(), "F5");
    assert!(c.has_command_mod() && Chord::alt(KeyName::Up).has_command_mod());
    assert!(!Chord::shift(KeyName::F(7)).has_command_mod());
}

#[test]
fn mac_and_pc_labels() {
    use super::chord::KeyLabels::{self, Mac, Pc};
    use KeyName as K;
    assert_eq!(Chord::cmd_alt(K::Digit(1)).text_in(Mac), "Cmd+Option+1");
    assert_eq!(Chord::alt(K::Down).text_in(Mac), "Option+Down");
    assert_eq!(Chord::ctrl(K::Tab).text_in(Mac), "Ctrl+Tab");
    assert_eq!(Chord::cmd_shift(K::Letter('P')).text_in(Mac), "Cmd+Shift+P");
    assert_eq!(Chord::cmd(K::Comma).text_in(Mac), "Cmd+,");
    assert_eq!(Chord::cmd_alt(K::Digit(1)).text_in(Pc), "Ctrl+Alt+1");
    assert_eq!(Chord::alt(K::Down).text_in(Pc), "Alt+Down");
    assert_eq!(Chord::cmd_shift(K::Letter('P')).text_in(Pc), "Ctrl+Shift+P");
    assert_eq!(Chord::ctrl(K::Tab).text_in(Pc), "Ctrl+Tab");
    let full = Chord::cmd_ctrl(K::Letter('F'));
    assert_eq!(full.text_in(Mac), "Cmd+Ctrl+F");
    assert_eq!(full.config(), "Mod+Ctrl+F");
    assert_eq!(Chord::parse(&full.config()), Some(full));
    let m = Chord::cmd_shift(K::Letter('M'));
    assert_eq!(m.parts_in(Mac), ["Cmd", "Shift", "M"]);
    assert_eq!(m.text(), m.text_in(KeyLabels::native()));
}
