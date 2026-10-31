//! The Repository commands rows: stable ids after the older ones, no default keys, the usual
//! rebinding rules, loading and saving their keys, and the shortcut sheet leaving out the rows
//! that have no key.

use super::rebind::Refusal;
use super::*;
use std::collections::BTreeMap;

use BindingId as I;
use KeyName as K;

/// How many ids there were before the Repository commands rows.
const OLDER: usize = 76;

#[test]
fn command_ids_are_stable() {
    // Users' settings files name bindings by these: renaming one loses their changes.
    const FROZEN: [&str; 14] = [
        "fetch",
        "pull",
        "push",
        "create-branch",
        "create-tag",
        "create-annotated-tag",
        "stash",
        "stash-with-message",
        "pop-stash",
        "file-history",
        "blame-file",
        "open-journal",
        "open-in-terminal",
        "show-repo-folder",
    ];
    let commands = &BindingId::ALL[OLDER..OLDER + FROZEN.len()];
    let keys: Vec<&str> = commands.iter().map(|id| id.key()).collect();
    assert_eq!(keys, FROZEN);
    for id in COMMAND_BINDINGS {
        assert_eq!(BindingId::from_key(id.key()), Some(id));
    }
    assert_eq!(COMMAND_BINDINGS[..], *commands);
}

#[test]
fn command_rows_have_no_default_key() {
    let keys = Keymap::default();
    for id in COMMAND_BINDINGS {
        let b = binding(id);
        assert!(b.rebindable && runs_command(id), "{id:?}");
        assert_eq!((b.group, b.when), (Group::Commands, When::NotTyping), "{id:?}");
        assert!(b.defaults.is_empty(), "{id:?}");
        assert_eq!(keys.hint(id), None, "{id:?}");
        assert_eq!(keys.keys_text(id), "No key", "{id:?}");
        assert!(keys.is_default(id), "{id:?}");
    }
    assert!(!runs_command(I::Refresh) && !runs_command(I::OpenFavorite1));
    let rows: Vec<BindingId> = BINDINGS.iter().filter(|b| b.group == Group::Commands).map(|b| b.id).collect();
    assert_eq!(rows, COMMAND_BINDINGS, "the group's rows, in order");
    let at = |g| GROUPS.iter().position(|&x| x == g).expect("listed");
    assert_eq!(at(Group::Commands), at(Group::Repositories) + 1);
    assert_eq!(Group::Commands.title(), "Repository commands");
}

#[test]
fn command_keys_follow_the_rebind_rules() {
    let keys = Keymap::default();
    let fold = fold_ctrl();
    assert_eq!(check(&keys, I::Fetch, Chord::plain(K::F(8)), fold), Check::Ok, "plain F-keys are allowed");
    // A plain letter clashes with the browsing keys; S is the rebase panel's fixed squash first.
    assert_eq!(check(&keys, I::Fetch, Chord::plain(K::Letter('U')), fold), Check::Clash { other: I::Unstage, fixed: false });
    assert_eq!(check(&keys, I::Fetch, Chord::plain(K::Letter('S')), fold), Check::Clash { other: I::RebaseSquash, fixed: true });
    assert_eq!(check(&keys, I::Pull, Chord::plain(K::Letter('P')), fold), Check::Clash { other: I::RebasePick, fixed: true });
    assert_eq!(check(&keys, I::Push, Chord::cmd_shift(K::Letter('S')), fold), Check::Clash { other: I::StageAll, fixed: false });
    assert_eq!(check(&keys, I::Push, Chord::plain(K::Escape), fold), Check::Refused(Refusal::Reserved));
    assert_eq!(check(&keys, I::Push, Chord::plain(K::Down), fold), Check::Refused(Refusal::Reserved));
    assert_eq!(check(&keys, I::Push, Chord::cmd(K::Letter('C')), fold), Check::Refused(Refusal::ClipboardKey));
}

fn map(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    entries.iter().map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect())).collect()
}

#[test]
fn command_overrides_round_trip() {
    let saved = map(&[("fetch", &["F8"]), ("open-in-terminal", &["Mod+Alt+T"])]);
    let (keys, problems) = Keymap::from_overrides(&saved);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys.chords(I::Fetch), [Chord::plain(K::F(8))]);
    assert_eq!(keys.chords(I::OpenTerminal), [Chord::cmd_alt(K::Letter('T'))]);
    assert_eq!(keys.hint(I::Fetch), Some("F8"));
    assert_eq!(keys.overrides(), saved);
    assert_eq!(keys.lookup(Chord::plain(K::F(8)), fold_ctrl()), Some(I::Fetch));

    // No key is the default, so an empty list changes nothing and isn't written back.
    let (keys, problems) = Keymap::from_overrides(&map(&[("fetch", &[])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys, Keymap::default());
    assert!(keys.overrides().is_empty());

    // A changed binding claims its key first, even a default key of another.
    let (keys, problems) = Keymap::from_overrides(&map(&[("push", &["Mod+Shift+S"])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys.chords(I::Push), [Chord::cmd_shift(K::Letter('S'))]);
    assert!(keys.chords(I::StageAll).is_empty());
}

fn group_ids(groups: &[(Group, Vec<&'static Binding>)], g: Group) -> Option<Vec<BindingId>> {
    groups.iter().find(|(x, _)| *x == g).map(|(_, rows)| rows.iter().map(|b| b.id).collect())
}

#[test]
fn sheet_leaves_out_unbound_commands() {
    let mut keys = Keymap::default();
    assert_eq!(group_ids(&keys.filter_for_sheet(""), Group::Commands), None);
    assert_eq!(group_ids(&keys.filter(""), Group::Commands), Some(COMMAND_BINDINGS.to_vec()));
    keys.set(I::Fetch, vec![Chord::plain(K::F(8))]);
    assert_eq!(group_ids(&keys.filter_for_sheet(""), Group::Commands), Some(vec![I::Fetch]));
    keys.set(I::NewTab, Vec::new());
    let tabs = group_ids(&keys.filter_for_sheet(""), Group::Tabs).expect("the Tabs group");
    assert!(tabs.contains(&I::NewTab), "other rows without a key still show");
}
