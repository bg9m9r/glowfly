//! The binding table: one row per id, chord text, no clashing global keys, and filtering.

use super::*;

const ALL: [BindingId; 32] = [
    I::Stage,
    I::Unstage,
    I::StageAll,
    I::UnstageAll,
    I::FocusMessage,
    I::Commit,
    I::StageAllAndCommit,
    I::SelectRow,
    I::NextChange,
    I::PrevChange,
    I::CloseView,
    I::NewTab,
    I::CloseTab,
    I::ReopenTab,
    I::NextTab,
    I::PrevTab,
    I::GoToTab,
    I::LastTab,
    I::Palette,
    I::PaletteMove,
    I::PaletteRun,
    I::PaletteBack,
    I::PaletteClose,
    I::Sheet,
    I::Search,
    I::SearchNext,
    I::Prefs,
    I::Refresh,
    I::Undo,
    I::Redo,
    I::DialogSubmit,
    I::ToggleTerminal,
];

#[test]
fn every_id_has_exactly_one_row() {
    for id in ALL {
        assert_eq!(BINDINGS.iter().filter(|b| b.id == id).count(), 1, "{id:?}");
        assert_eq!(binding(id).id, id);
        assert!(!binding(id).chords.is_empty(), "{id:?} has no keys");
    }
    assert_eq!(BINDINGS.len(), ALL.len(), "a row's id is missing from ALL");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn chord_text() {
    assert_eq!(hint(I::StageAll), "Ctrl+Shift+S");
    assert_eq!(hint(I::Stage), "S");
    assert_eq!(hint(I::PrevChange), "Shift+F7");
    assert_eq!(hint(I::Sheet), "Ctrl+/");
    assert_eq!(hint(I::Prefs), "Ctrl+,");
    assert_eq!(hint(I::Redo), "Ctrl+Y");
    assert_eq!(hint(I::Palette), "Ctrl+P");
    assert_eq!(hint(I::NextTab), "Ctrl+Tab");
    assert_eq!(hint(I::CloseView), "Esc");
    assert_eq!(keys_text(binding(I::NextChange)), "F7 or Alt+Down");
    assert_eq!(Chord::cmd_shift(KeyName::Letter('M')).parts(), ["Ctrl", "Shift", "M"]);
}

/// The chords a global binding answers to, with Cmd/Ctrl folded together (they are the same
/// key off macOS) and a digit range expanded.
fn global_chords(b: &Binding) -> Vec<(bool, bool, bool, KeyName)> {
    let mut out = Vec::new();
    for c in b.chords {
        let ctrl = c.mods.command || c.mods.ctrl;
        match c.key {
            KeyName::DigitRange(lo, hi) => out.extend((lo..=hi).map(|d| (ctrl, c.mods.shift, c.mods.alt, KeyName::Digit(d)))),
            k => out.push((ctrl, c.mods.shift, c.mods.alt, k)),
        }
    }
    out
}

#[test]
fn global_keys_never_clash() {
    let mut seen: Vec<((bool, bool, bool, KeyName), BindingId)> = Vec::new();
    for b in BINDINGS.iter().filter(|b| matches!(b.when, When::Anywhere | When::NotTyping)) {
        for chord in global_chords(b) {
            if let Some((_, other)) = seen.iter().find(|(c, _)| *c == chord) {
                panic!("{:?} and {other:?} share {chord:?}", b.id);
            }
            seen.push((chord, b.id));
        }
    }
    assert!(seen.iter().any(|(c, id)| *id == I::GoToTab && c.3 == KeyName::Digit(8)));
    assert!(seen.iter().any(|(c, id)| *id == I::LastTab && c.3 == KeyName::Digit(9)));
}

fn ids(groups: &[(Group, Vec<&'static Binding>)]) -> Vec<BindingId> {
    groups.iter().flat_map(|(_, rows)| rows.iter().map(|b| b.id)).collect()
}

#[test]
fn filter_by_label_key_and_note() {
    let all = filter("");
    assert_eq!(all.iter().map(|(g, _)| *g).collect::<Vec<_>>(), GROUPS);
    assert_eq!(ids(&all).len(), BINDINGS.len());
    assert_eq!(filter("   ").len(), GROUPS.len(), "blank terms match everything");

    let stage = filter("stage");
    assert_eq!(stage.iter().map(|(g, _)| *g).collect::<Vec<_>>(), [Group::Staging]);
    assert_eq!(ids(&stage), [I::Stage, I::Unstage, I::StageAll, I::UnstageAll, I::StageAllAndCommit]);
    assert_eq!(ids(&filter("STAGE")), ids(&stage), "case is ignored");

    assert_eq!(ids(&filter("f7")), [I::NextChange, I::PrevChange]);
    let note = ids(&filter("in the commit message"));
    assert!(note.contains(&I::Commit) && note.contains(&I::StageAllAndCommit), "{note:?}");
    assert!(filter("zzz").is_empty());
    assert_eq!(ids(&filter("tabs")).len(), 7, "the group title matches");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn filter_terms_all_have_to_match() {
    let found = ids(&filter("ctrl shift"));
    assert!(found.contains(&I::StageAll) && found.contains(&I::ReopenTab), "{found:?}");
    assert!(!found.contains(&I::NewTab), "{found:?}");
    assert_eq!(ids(&filter("ctrl+shift+s")), [I::StageAll]);
}
