//! The Repository commands rows: stable ids after the older ones, the seven default keys (and
//! no key for the rest), the usual rebinding rules, loading and saving their keys, a user's
//! binding keeping a key that is now a default, and the shortcut sheet leaving out the rows
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
    // Cherry-pick and Revert came later, then Switch branch and Open dashboard, before the six
    // conflict keys: the last ids.
    let later = [I::CherryPick, I::RevertCommit, I::SwitchBranch, I::OpenDashboard];
    assert_eq!(COMMAND_BINDINGS[..FROZEN.len()], *commands);
    assert_eq!(COMMAND_BINDINGS[FROZEN.len()..], later);
    let n = BindingId::ALL.len() - 6;
    assert_eq!(BindingId::ALL[n - later.len()..n], later);
    let names: Vec<&str> = later.iter().map(|id| id.key()).collect();
    assert_eq!(names, ["cherry-pick", "revert-commit", "switch-branch", "open-dashboard"]);
}

/// The Repository commands rows that have keys by default, with them.
fn keyed() -> [(BindingId, Chord); 7] {
    [
        (I::Fetch, Chord::shift(K::F(5))),
        (I::Pull, Chord::cmd_shift(K::Down)),
        (I::Push, Chord::cmd_shift(K::Up)),
        (I::CreateBranch, Chord::cmd(K::Letter('B'))),
        (I::StashChanges, Chord::cmd_shift(K::Letter('H'))),
        (I::PopStash, Chord::cmd_alt_shift(K::Letter('H'))),
        (I::SwitchBranch, Chord::cmd_shift(K::Letter('O'))),
    ]
}

#[test]
fn command_rows_and_their_default_keys() {
    let keys = Keymap::default();
    for id in COMMAND_BINDINGS {
        let b = binding(id);
        assert!(b.rebindable && runs_command(id), "{id:?}");
        assert_eq!((b.group, b.when), (Group::Commands, When::NotTyping), "{id:?}");
        assert!(keys.is_default(id), "{id:?}");
        match keyed().iter().find(|(k, _)| *k == id) {
            Some(&(_, c)) => {
                assert_eq!(b.defaults, [c], "{id:?}");
                assert_eq!(keys.hint(id), Some(c.text().as_str()), "{id:?}");
            }
            None => {
                assert!(b.defaults.is_empty(), "{id:?}");
                assert_eq!(keys.hint(id), None, "{id:?}");
                assert_eq!(keys.keys_text(id), "No key", "{id:?}");
            }
        }
    }
    let labels = [
        (I::CreateBranch, "New branch…"),
        (I::CreateTag, "New tag…"),
        (I::CreateAnnotatedTag, "New annotated tag…"),
        (I::PopStash, "Unstash the latest stash"),
        (I::SwitchBranch, "Switch branch…"),
        (I::OpenDashboard, "Open dashboard"),
    ];
    for (id, label) in labels {
        assert_eq!(binding(id).label, label, "{id:?}");
    }
    assert!(!runs_command(I::Refresh) && !runs_command(I::OpenFavorite1));
    let rows: Vec<BindingId> = BINDINGS.iter().filter(|b| b.group == Group::Commands).map(|b| b.id).collect();
    assert_eq!(rows, COMMAND_BINDINGS, "the group's rows, in order");
    let at = |g| GROUPS.iter().position(|&x| x == g).expect("listed");
    assert_eq!(at(Group::Commands), at(Group::Repositories) + 1);
    assert_eq!(Group::Commands.title(), "Repository commands");
}

/// The seven defaults clash with no other key that works at the same time (`When::overlaps`),
/// with Cmd and Ctrl folded into one (Windows and Linux) and apart (macOS), on any system.
#[test]
fn command_defaults_overlap_no_other_key() {
    for fold in [true, false] {
        for (id, c) in keyed() {
            let when = binding(id).when;
            for b in BINDINGS.iter().filter(|b| b.id != id && b.when.overlaps(when)) {
                let clash = b.defaults.iter().any(|k| same_keys(*k, c, fold));
                assert!(!clash, "{id:?} {c:?} clashes with {:?} (fold {fold})", b.id);
            }
            assert_eq!(super::rebind::refusal(id, c), None, "{id:?} {c:?}");
        }
    }
    let keys = Keymap::default();
    for (id, c) in keyed() {
        assert_eq!(keys.lookup(c, fold_ctrl()), Some(id), "{c:?}");
        assert_eq!(check(&keys, id, c, fold_ctrl()), Check::Same, "{id:?}");
    }
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
    assert_eq!(check(&keys, I::Push, Chord::cmd(K::Letter('B')), fold), Check::Clash { other: I::CreateBranch, fixed: false });
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

    // Open in terminal has no key by default, so an empty list changes nothing and isn't
    // written back.
    let (keys, problems) = Keymap::from_overrides(&map(&[("open-in-terminal", &[])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys, Keymap::default());
    assert!(keys.overrides().is_empty());

    // A changed binding claims its key first, even a default key of another.
    let (keys, problems) = Keymap::from_overrides(&map(&[("push", &["Mod+Shift+S"])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys.chords(I::Push), [Chord::cmd_shift(K::Letter('S'))]);
    assert!(keys.chords(I::StageAll).is_empty());
}

/// A user's saved binding on a key that is now one of the new defaults keeps it; the new
/// default stays unbound, and nothing is reported.
#[test]
fn a_user_binding_keeps_a_key_that_is_now_a_default() {
    let (keys, problems) = Keymap::from_overrides(&map(&[("open-journal", &["Mod+B"])]));
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keys.chords(I::OpenJournal), [Chord::cmd(K::Letter('B'))]);
    assert!(keys.chords(I::CreateBranch).is_empty(), "New branch has no key");
    assert_eq!(keys.keys_text(I::CreateBranch), "No key");
    assert_eq!(keys.lookup(Chord::cmd(K::Letter('B')), fold_ctrl()), Some(I::OpenJournal));
    // The other new defaults keep theirs.
    assert_eq!(keys.chords(I::StashChanges), [Chord::cmd_shift(K::Letter('H'))]);
}

fn group_ids(groups: &[(Group, Vec<&'static Binding>)], g: Group) -> Option<Vec<BindingId>> {
    groups.iter().find(|(x, _)| *x == g).map(|(_, rows)| rows.iter().map(|b| b.id).collect())
}

#[test]
fn sheet_leaves_out_unbound_commands() {
    let mut keys = Keymap::default();
    let with_key = |keys: &Keymap| -> Vec<BindingId> { COMMAND_BINDINGS.into_iter().filter(|&id| !keys.chords(id).is_empty()).collect() };
    let defaults: Vec<BindingId> = keyed().iter().map(|(id, _)| *id).collect();
    assert_eq!(with_key(&keys), defaults, "the seven, in table order");
    assert_eq!(group_ids(&keys.filter_for_sheet(""), Group::Commands), Some(defaults));
    assert_eq!(group_ids(&keys.filter(""), Group::Commands), Some(COMMAND_BINDINGS.to_vec()));
    keys.set(I::OpenJournal, vec![Chord::plain(K::F(8))]);
    keys.set(I::Fetch, Vec::new());
    let shown = group_ids(&keys.filter_for_sheet(""), Group::Commands).expect("the Commands group");
    assert_eq!(shown, with_key(&keys));
    assert!(shown.contains(&I::OpenJournal) && !shown.contains(&I::Fetch));
    keys.set(I::NewTab, Vec::new());
    let tabs = group_ids(&keys.filter_for_sheet(""), Group::Tabs).expect("the Tabs group");
    assert!(tabs.contains(&I::NewTab), "other rows without a key still show");
}
