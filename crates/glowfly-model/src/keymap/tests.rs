//! The binding table and the effective keymap: one row per id, stable ids, chord text, no
//! clashing defaults and filtering. Loading the user's changes from settings is in
//! `override_tests.rs`.

use super::*;

use BindingId as I;

#[test]
fn every_id_has_exactly_one_row() {
    for (n, id) in BindingId::ALL.into_iter().enumerate() {
        assert_eq!(id as usize, n, "ALL is in declaration order");
        assert_eq!(BINDINGS.iter().filter(|b| b.id == id).count(), 1, "{id:?}");
        assert_eq!(binding(id).id, id);
        assert!(runs_command(id) || !binding(id).defaults.is_empty(), "{id:?} has no keys");
    }
    assert_eq!(BINDINGS.len(), BindingId::ALL.len(), "a row's id is missing from ALL");
}

#[test]
fn ids_are_stable() {
    // Users' settings files name bindings by these: renaming one loses their changes.
    const FROZEN: [&str; 76] = [
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
        "extend-selection",
        "select-all-commits",
        "select-focused-commit",
        "open-fold",
        "close-fold",
        "toggle-fullscreen",
    ];
    let keys: Vec<&str> = BindingId::ALL.iter().map(|id| id.key()).collect();
    assert_eq!(keys[..FROZEN.len()], FROZEN);
    for id in BindingId::ALL {
        assert_eq!(BindingId::from_key(id.key()), Some(id));
    }
    assert_eq!(BindingId::from_key("New-Tab"), None);
    assert_eq!(BindingId::from_key("send-pull-request-text"), Some(BindingId::SendPrText));
    assert_eq!(
        BindingId::ALL[BindingId::ALL.len() - 31..BindingId::ALL.len() - 27],
        [BindingId::SendPrText, BindingId::Quit, BindingId::PrefsSave, BindingId::PrefsRevert]
    );
    assert_eq!(BindingId::from_key("wip-other-list"), Some(BindingId::WipOtherList));
    assert_eq!(BindingId::from_key("file-list-step-file"), Some(BindingId::FileStepFile));
    assert_eq!(BindingId::from_key("focus-sidebar"), Some(BindingId::FocusSidebar));
    assert_eq!(BindingId::from_key("copy-commit-ids"), Some(BindingId::CopyCommitIds));
    assert_eq!(BindingId::ALL.last(), Some(&BindingId::RevertCommit));
    assert_eq!(BindingId::from_key("save-preferences-page"), Some(BindingId::PrefsSave));
}

#[test]
fn rebindable_set() {
    let rebindable: Vec<BindingId> = BindingId::ALL.into_iter().filter(|&id| binding(id).rebindable && !runs_command(id)).collect();
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
        I::SelectAllCommits,
        I::SelectFocusedCommit,
        I::OpenFold,
        I::CloseFold,
        I::ToggleFullscreen,
        I::Quit,
        I::PrefsSave,
        I::FocusSidebar,
        I::FocusCentral,
        I::FocusFiles,
        I::FocusDiff,
        I::FocusCommitMessage,
        I::NextPane,
        I::PrevPane,
        I::SelectHead,
        I::PrevRefCommit,
        I::NextRefCommit,
        I::CopyCommitIds,
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
    assert_eq!(keys.keys_text(I::OpenFold), "E");
    assert_eq!(keys.keys_text(I::CloseFold), "Shift+E");
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
    assert_eq!(keys.hint(I::ToggleFullscreen), Some("F11"));
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

pub(super) fn ids(groups: &[(Group, Vec<&'static Binding>)]) -> Vec<BindingId> {
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
    // The WIP lists' rows match by their note ("in the Unstaged and Staged Files lists").
    let wip = [
        I::WipStepFile,
        I::WipExtendFiles,
        I::WipPageFile,
        I::WipEdgeFile,
        I::WipToggleFiles,
        I::WipOpenFile,
        I::WipDiscardFile,
        I::WipOtherList,
    ];
    assert_eq!(ids(&stage), [[I::Stage, I::Unstage, I::StageAll, I::UnstageAll, I::StageAllAndCommit].as_slice(), &wip].concat());
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

#[test]
fn wip_list_keys_are_fixed_and_kept_apart() {
    for b in BINDINGS.iter().filter(|b| b.when == When::WipLists) {
        assert!(!b.rebindable, "{:?}", b.id);
    }
    // A focused list takes its keys before any binding: only bindings that work anywhere a list
    // can have the focus can't share them.
    for other in [When::Graph, When::Diff, When::WorkingDiff, When::Lists, When::Blame, When::Message, When::RebasePanel] {
        assert!(!When::WipLists.overlaps(other) && !other.overlaps(When::WipLists), "{other:?}");
    }
    assert!(When::WipLists.overlaps(When::NotTyping) && When::NotTyping.overlaps(When::WipLists));
    assert!(When::WipLists.overlaps(When::Anywhere) && When::WipLists.overlaps(When::WipLists));
}

#[test]
fn file_list_keys_are_fixed_and_kept_apart() {
    for b in BINDINGS.iter().filter(|b| b.when == When::FileLists) {
        assert!(!b.rebindable, "{:?}", b.id);
    }
    // Only one list has the focus, so these share their keys with the WIP lists'.
    for other in [When::Graph, When::Diff, When::Lists, When::Blame, When::Message, When::RebasePanel, When::WipLists] {
        assert!(!When::FileLists.overlaps(other) && !other.overlaps(When::FileLists), "{other:?}");
    }
    assert!(When::FileLists.overlaps(When::NotTyping) && When::FileLists.overlaps(When::Anywhere));
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
