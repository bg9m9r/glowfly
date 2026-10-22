//! The Quit key: a rebindable General row that works anywhere, Mod+Q by default (Ctrl+Q on
//! Windows and Linux, Cmd+Q on macOS), with a stable settings-file name.

use super::chord::KeyLabels;
use super::*;

use BindingId as I;

#[test]
fn quit_is_a_general_key_that_works_anywhere() {
    let b = binding(I::Quit);
    assert!(b.rebindable);
    assert_eq!(b.group, Group::General);
    assert_eq!(b.when, When::Anywhere);
    assert_eq!(b.label, "Quit Glowfly");
    assert_eq!(b.defaults, [Chord::cmd(KeyName::Letter('Q'))]);
    assert_eq!(BindingId::ALL.last(), Some(&I::Quit), "appended, so older ids keep their place");
}

#[test]
fn quit_keeps_its_settings_name() {
    assert_eq!(I::Quit.key(), "quit");
    assert_eq!(BindingId::from_key("quit"), Some(I::Quit));
}

#[test]
fn quit_reads_as_each_platform_names_it() {
    let chord = binding(I::Quit).defaults[0];
    assert_eq!(chord.text_in(KeyLabels::Pc), "Ctrl+Q");
    assert_eq!(chord.text_in(KeyLabels::Mac), "Cmd+Q");
    assert_eq!(chord.config(), "Mod+Q");
    assert_eq!(Keymap::default().lookup(Chord::cmd(KeyName::Letter('Q')), fold_ctrl()), Some(I::Quit));
}
