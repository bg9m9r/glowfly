//! The binding table and the effective keymap: one row per id, stable ids, chord text, no
//! clashing defaults, filtering, and loading the user's changes from settings.

use super::*;
use std::collections::BTreeMap;

use BindingId as I;

#[test]
fn every_id_has_exactly_one_row() {
    for (n, id) in BindingId::ALL.into_iter().enumerate() {
        assert_eq!(id as usize, n, "ALL is in declaration order");
        assert_eq!(BINDINGS.iter().filter(|b| b.id == id).count(), 1, "{id:?}");
        assert_eq!(binding(id).id, id);
        assert!(!binding(id).defaults.is_empty(), "{id:?} has no keys");
    }
    assert_eq!(BINDINGS.len(), BindingId::ALL.len(), "a row's id is missing from ALL");
}

#[test]
fn ids_are_stable() {
    // Users' settings files name bindings by these: renaming one loses their changes.
    const FROZEN: [&str; 70] = [
        "stage",
        "unstage",
        "stage-all",
        "unstage-all",
        "focus-message",
        "commit",
        "stage-all-and-commit",
        "select-row",
        "next-change",
        "prev-change",
        "close-view",
        "new-tab",
        "close-tab",
        "reopen-tab",
        "next-tab",
        "prev-tab",
        "go-to-tab",
        "last-tab",
        "move-tab-left",
        "move-tab-right",
        "palette",
        "palette-move",
        "palette-run",
        "palette-back",
        "palette-close",
        "shortcut-sheet",
        "search",
        "search-next",
        "search-filter",
        "preferences",
        "refresh",
        "undo",
        "redo",
        "dialog-submit",
        "toggle-terminal",
        "zoom-in",
        "zoom-out",
        "zoom-reset",
        "toggle-left-panel",
        "toggle-details-panel",
        "toggle-toolbar",
        "blame-next-run",
        "blame-prev-run",
        "open-favorite-1",
        "open-favorite-2",
        "open-favorite-3",
        "open-favorite-4",
        "open-favorite-5",
        "open-favorite-6",
        "open-favorite-7",
        "open-favorite-8",
        "open-favorite-9",
        "reword-commit",
        "squash-commit",
        "drop-commit",
        "rebase-continue",
        "rebase-skip",
        "rebase-step-select",
        "rebase-step-move",
        "rebase-pick",
        "rebase-reword",
        "rebase-squash",
        "rebase-fixup",
        "rebase-drop",
        "rebase-edit",
        "rebase-add-exec",
        "rebase-add-break",
        "rebase-remove-step",
        "rebase-edit-step",
        "revert-hunk",
    ];
    let keys: Vec<&str> = BindingId::ALL.iter().map(|id| id.key()).collect();
    assert_eq!(keys, FROZEN);
    for id in BindingId::ALL {
        assert_eq!(BindingId::from_key(id.key()), Some(id));
    }
    assert_eq!(BindingId::from_key("New-Tab"), None);
}

#[test]
fn rebindable_set() {
    let rebindable: Vec<BindingId> = BindingId::ALL.into_iter().filter(|&id| binding(id).rebindable).collect();
    let want = [
        I::Stage,
        I::Unstage,
        I::StageAll,
        I::UnstageAll,
        I::FocusMessage,
        I::NextChange,
        I::PrevChange,
        I::NewTab,
        I::CloseTab,
        I::ReopenTab,
        I::NextTab,
        I::PrevTab,
        I::LastTab,
        I::MoveTabLeft,
        I::MoveTabRight,
        I::Palette,
        I::Sheet,
        I::Search,
        I::SearchFilter,
        I::Prefs,
        I::Refresh,
        I::Undo,
        I::Redo,
        I::ToggleTerminal,
        I::ZoomIn,
        I::ZoomOut,
        I::ZoomReset,
        I::ToggleLeftPanel,
        I::ToggleDetailsPanel,
        I::ToggleToolbar,
        I::BlameNextRun,
        I::BlamePrevRun,
        I::OpenFavorite1,
        I::OpenFavorite2,
        I::OpenFavorite3,
        I::OpenFavorite4,
        I::OpenFavorite5,
        I::OpenFavorite6,
        I::OpenFavorite7,
        I::OpenFavorite8,
        I::OpenFavorite9,
        I::RewordCommit,
        I::SquashCommit,
        I::DropCommit,
        I::RebaseContinue,
        I::RebaseSkip,
        I::RevertHunk,
    ];
    assert_eq!(rebindable, want);
}

#[test]
fn favorite_keys_by_default() {
    let keys = Keymap::default();
    let fold = fold_ctrl();
    assert_eq!(keys.lookup(Chord::cmd_alt(KeyName::Digit(3)), fold), Some(I::OpenFavorite3));
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Digit(3)), fold), Some(I::GoToTab), "Ctrl+3 still goes to a tab");
    for (n, id) in FAVORITE_BINDINGS.into_iter().enumerate() {
        assert_eq!(id.favorite_slot(), Some(n));
        assert_eq!(binding(id).when, When::NotTyping, "AltGr+digit types characters");
        assert_eq!(binding(id).defaults, [Chord::cmd_alt(KeyName::Digit(n as u8 + 1))]);
    }
    assert_eq!(I::GoToTab.favorite_slot(), None);
    let found = keys.filter("favorite");
    assert_eq!(found.iter().map(|(g, _)| *g).collect::<Vec<_>>(), [Group::Repositories]);
    assert_eq!(found[0].1.len(), 9);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn chord_text() {
    let keys = Keymap::default();
    assert_eq!(keys.hint(I::StageAll), Some("Ctrl+Shift+S"));
    assert_eq!(keys.hint(I::Stage), Some("S"));
    assert_eq!(keys.hint(I::RevertHunk), Some("R"));
    assert_eq!(keys.hint(I::PrevChange), Some("Shift+F7"));
    assert_eq!(keys.hint(I::Sheet), Some("Ctrl+/"));
    assert_eq!(keys.hint(I::Prefs), Some("Ctrl+,"));
    assert_eq!(keys.hint(I::Redo), Some("Ctrl+Y"));
    assert_eq!(keys.hint(I::Palette), Some("Ctrl+P"));
    assert_eq!(keys.hint(I::NextTab), Some("Ctrl+Tab"));
    assert_eq!(keys.hint(I::MoveTabLeft), Some("Ctrl+Shift+PgUp"));
    assert_eq!(keys.hint(I::MoveTabRight), Some("Ctrl+Shift+PgDn"));
    assert_eq!(keys.hint(I::CloseView), Some("Esc"));
    assert_eq!(keys.keys_text(I::NextChange), "F7 or Alt+Down");
    assert_eq!(keys.tip("New tab", I::NewTab), "New tab (Ctrl+T)");
    assert_eq!(Chord::cmd_shift(KeyName::Letter('M')).parts(), ["Ctrl", "Shift", "M"]);
    assert_eq!(keys.hint(I::ZoomIn), Some("Ctrl+="));
    assert_eq!(keys.keys_text(I::ZoomIn), "Ctrl+= or Ctrl+Plus");
    assert_eq!(keys.hint(I::ZoomReset), Some("Ctrl+0"));
    assert_eq!(keys.hint(I::ToggleToolbar), Some("Ctrl+Shift+B"));
}

#[test]
fn zoom_keys_by_default() {
    let keys = Keymap::default();
    let fold = fold_ctrl();
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Equals), fold), Some(I::ZoomIn));
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Plus), fold), Some(I::ZoomIn));
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Minus), fold), Some(I::ZoomOut));
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Digit(0)), fold), Some(I::ZoomReset));
    assert_eq!(keys.lookup(Chord::cmd_shift(KeyName::Letter('L')), fold), Some(I::ToggleLeftPanel));
    assert_eq!(keys.lookup(Chord::cmd_shift(KeyName::Letter('D')), fold), Some(I::ToggleDetailsPanel));
    assert_eq!(keys.lookup(Chord::cmd_shift(KeyName::Letter('B')), fold), Some(I::ToggleToolbar));
    assert_eq!(keys.lookup(Chord::plain(KeyName::Letter('N')), fold), Some(I::BlameNextRun));
    assert_eq!(keys.lookup(Chord::shift(KeyName::Letter('N')), fold), Some(I::BlamePrevRun));
    let view: Vec<BindingId> = BINDINGS.iter().filter(|b| b.group == Group::View).map(|b| b.id).collect();
    assert!(view.contains(&I::ToggleTerminal) && view.contains(&I::ZoomIn), "{view:?}");
}

#[test]
fn global_keys_never_clash() {
    let keys = Keymap::default();
    let fold = fold_ctrl();
    for b in BINDINGS.iter().filter(|b| b.rebindable) {
        for &c in b.defaults {
            assert_eq!(check(&keys, b.id, c, fold), Check::Same, "{:?} {c:?}", b.id);
            // Taken away from itself, each default key would be free for it again.
            let mut without = keys.clone();
            without.set(b.id, Vec::new());
            assert_eq!(check(&without, b.id, c, fold), Check::Ok, "{:?} {c:?} clashes", b.id);
        }
    }
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Digit(8)), fold), Some(I::GoToTab));
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Digit(9)), fold), Some(I::LastTab));
}

fn ids(groups: &[(Group, Vec<&'static Binding>)]) -> Vec<BindingId> {
    groups.iter().flat_map(|(_, rows)| rows.iter().map(|b| b.id)).collect()
}

#[test]
fn filter_by_label_key_and_note() {
    let keys = Keymap::default();
    let all = keys.filter("");
    assert_eq!(all.iter().map(|(g, _)| *g).collect::<Vec<_>>(), GROUPS);
    assert_eq!(ids(&all).len(), BINDINGS.len());
    assert_eq!(keys.filter("   ").len(), GROUPS.len(), "blank terms match everything");

    let stage = keys.filter("stage");
    assert_eq!(stage.iter().map(|(g, _)| *g).collect::<Vec<_>>(), [Group::Staging]);
    assert_eq!(ids(&stage), [I::Stage, I::Unstage, I::StageAll, I::UnstageAll, I::StageAllAndCommit]);
    assert_eq!(ids(&keys.filter("STAGE")), ids(&stage), "case is ignored");

    assert_eq!(ids(&keys.filter("f7")), [I::NextChange, I::PrevChange]);
    let note = ids(&keys.filter("in the commit message"));
    assert!(note.contains(&I::Commit) && note.contains(&I::StageAllAndCommit), "{note:?}");
    assert!(keys.filter("zzz").is_empty());
    assert_eq!(ids(&keys.filter("tabs")).len(), 9, "the group title matches");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn filter_terms_all_have_to_match() {
    let keys = Keymap::default();
    let found = ids(&keys.filter("ctrl shift"));
    assert!(found.contains(&I::StageAll) && found.contains(&I::ReopenTab), "{found:?}");
    assert!(!found.contains(&I::NewTab), "{found:?}");
    assert_eq!(ids(&keys.filter("ctrl+shift+s")), [I::StageAll]);
}

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
    assert_eq!(k.defaults_text(I::NextChange), "F7 or Alt+Down", "the defaults, not the current keys");
    assert_eq!(k.keys_text(I::NextChange), "F8");
}

#[test]
fn rebase_panel_keys_are_fixed_and_kept_apart() {
    let keys = Keymap::default();
    let fold = fold_ctrl();
    // The panel's keys work while nothing is typed, so a key for NotTyping can't take them.
    let p = Chord::plain(KeyName::Letter('P'));
    assert_eq!(check(&keys, I::Undo, p, fold), Check::Clash { other: I::RebasePick, fixed: true });
    // The panel is never drawn with a diff, a list, the graph or blame, so their keys may match.
    for other in [When::DiffOrConflict, When::Lists, When::Graph, When::Blame, When::WorkingDiff] {
        assert!(!When::RebasePanel.overlaps(other) && !other.overlaps(When::RebasePanel), "{other:?}");
    }
    assert!(When::RebasePanel.overlaps(When::NotTyping) && When::NotTyping.overlaps(When::RebasePanel));
    assert!(When::RebasePanel.overlaps(When::Anywhere) && When::RebasePanel.overlaps(When::RebasePanel));
    assert!(When::Graph.overlaps(When::Lists) && When::Graph.overlaps(When::NotTyping));
    let history: Vec<BindingId> = BINDINGS.iter().filter(|b| b.group == Group::History).map(|b| b.id).collect();
    assert_eq!(history.len(), 17, "{history:?}");
    assert!(BINDINGS.iter().filter(|b| b.when == When::RebasePanel).all(|b| !b.rebindable));
    assert_eq!(keys.lookup(Chord::plain(KeyName::F(2)), fold), Some(I::RewordCommit));
    assert_eq!(keys.lookup(Chord::alt(KeyName::Letter('S')), fold), Some(I::SquashCommit));
    assert_eq!(keys.lookup(Chord::plain(KeyName::Delete), fold), Some(I::DropCommit));
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Backspace), fold), Some(I::DropCommit));
    assert_eq!(keys.lookup(Chord::alt(KeyName::Letter('C')), fold), Some(I::RebaseContinue));
    assert_eq!(keys.lookup(Chord::alt(KeyName::Letter('K')), fold), Some(I::RebaseSkip));
}
