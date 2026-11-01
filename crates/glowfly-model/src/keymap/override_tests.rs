//! Loading the user's keyboard changes from settings: no overrides is the defaults, an override
//! changes the keys, hint and tip, bad entries are ignored one line each, an override takes a
//! default key from the binding that had it, and the changes round-trip.

use super::tests::ids;
use super::*;
use std::collections::BTreeMap;

use BindingId as I;

fn map(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    entries.iter().map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect())).collect()
}

#[test]
fn no_overrides_is_the_defaults() {
    let (keys, problems) = Keymap::from_overrides(&BTreeMap::new());
    assert_eq!(keys, Keymap::default());
    assert!(problems.is_empty());
    assert!(!keys.customized());
    assert!(keys.overrides().is_empty());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn an_override_changes_keys_hint_and_tip() {
    let (keys, problems) = Keymap::from_overrides(&map(&[("new-tab", &["Mod+Shift+N"])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys.chords(I::NewTab), [Chord::cmd_shift(KeyName::Letter('N'))]);
    assert_eq!(keys.hint(I::NewTab), Some("Ctrl+Shift+N"));
    assert_eq!(keys.tip("New tab", I::NewTab), "New tab (Ctrl+Shift+N)");
    assert!(keys.customized() && !keys.is_default(I::NewTab) && keys.is_default(I::CloseTab));
}

#[test]
fn an_empty_list_leaves_no_key() {
    let (keys, problems) = Keymap::from_overrides(&map(&[("new-tab", &[])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert!(keys.chords(I::NewTab).is_empty());
    assert_eq!(keys.hint(I::NewTab), None);
    assert_eq!(keys.tip("New tab", I::NewTab), "New tab");
    assert_eq!(keys.paren(I::NewTab), "");
    assert_eq!(keys.keys_text(I::NewTab), "No key");
}

#[test]
fn bad_entries_are_ignored_one_line_each() {
    for (entries, what) in [
        (map(&[("no-such-thing", &["F9"])]), "an unknown id"),
        (map(&[("close-view", &["F9"])]), "a fixed id"),
        (map(&[("refresh", &["Mod+Nope"])]), "an unparsable chord"),
        (map(&[("refresh", &["Enter"])]), "a reserved chord"),
        (map(&[("palette", &["P"])]), "a plain letter for an Anywhere binding"),
        (map(&[("refresh", &["Mod+3"])]), "a fixed binding's key"),
    ] {
        let (keys, problems) = Keymap::from_overrides(&entries);
        assert_eq!(problems.len(), 1, "{what}: {problems:?}");
        assert!(problems[0].starts_with("Ignored keyboard setting "), "{what}: {problems:?}");
        assert_eq!(keys.chords(I::CloseView), binding(I::CloseView).defaults, "{what}");
        assert_eq!(keys.chords(I::Palette), binding(I::Palette).defaults, "{what}");
    }
}

#[test]
fn an_override_takes_a_default_key() {
    let (keys, problems) = Keymap::from_overrides(&map(&[("refresh", &["Mod+T"])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys.chords(I::Refresh), [Chord::cmd(KeyName::Letter('T'))]);
    assert!(keys.chords(I::NewTab).is_empty(), "the changed binding keeps it");
}

#[test]
fn two_overrides_with_one_key_keep_the_first_in_table_order() {
    let (keys, problems) = Keymap::from_overrides(&map(&[("undo", &["F9"]), ("refresh", &["F9"])]));
    assert_eq!(keys.chords(I::Refresh), [Chord::plain(KeyName::F(9))]);
    assert!(keys.chords(I::Undo).is_empty());
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn overrides_round_trip() {
    let mut keys = Keymap::default();
    keys.set(I::NewTab, vec![Chord::cmd_shift(KeyName::Letter('N'))]);
    keys.set(I::Refresh, vec![Chord::cmd(KeyName::Letter('R')), Chord::plain(KeyName::F(5))]);
    keys.set(I::CloseView, Vec::new());
    let saved = keys.overrides();
    assert_eq!(saved, map(&[("new-tab", &["Mod+Shift+N"]), ("refresh", &["Mod+R", "F5"])]), "only changed rebindable ids");
    let (back, problems) = Keymap::from_overrides(&saved);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(back, keys);
    keys.reset_all();
    assert_eq!(keys, Keymap::default());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn filter_follows_rebinding() {
    let (keys, _) = Keymap::from_overrides(&map(&[("refresh", &["Mod+R"])]));
    assert_eq!(ids(&keys.filter("ctrl+r")), [I::Refresh]);
    assert!(ids(&keys.filter("f5")).is_empty());
}

#[test]
fn defaults_text_names_the_default_keys() {
    let mut k = Keymap::default();
    k.set(I::NextChange, vec![Chord::plain(KeyName::F(8))]);
    // Alt reads "Option" on macOS.
    let alt = super::chord::KeyLabels::native().alt();
    assert_eq!(k.defaults_text(I::NextChange), format!("F7 or {alt}+Down"), "the defaults, not the current keys");
    assert_eq!(k.keys_text(I::NextChange), "F8");
}
