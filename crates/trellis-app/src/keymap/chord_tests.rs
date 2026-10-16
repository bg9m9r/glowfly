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
