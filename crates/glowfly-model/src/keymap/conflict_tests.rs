//! The conflict view's keys (`When::Conflict`): where they overlap, so their keys can't clash
//! with a binding that can be live at the same time, and the defaults' places and group.

use super::*;

use BindingId as I;
use When as W;

#[test]
fn the_conflict_place_overlaps_the_browsing_places_but_not_the_graph_or_text_fields() {
    for w in [W::Anywhere, W::NotTyping, W::WorkingDiff, W::CommitDiff, W::DiffOrConflict, W::Conflict, W::Diff, W::Lists, W::Blame] {
        assert!(W::Conflict.overlaps(w) && w.overlaps(W::Conflict), "{w:?}");
    }
    // The graph is never the central view with the conflict view open; the rest have their own
    // text field or take their keys first.
    for w in [
        W::Graph,
        W::Message,
        W::Search,
        W::InPalette,
        W::PaletteStep,
        W::Dialog,
        W::PrText,
        W::Prefs,
        W::RebasePanel,
        W::WipLists,
        W::FileLists,
    ] {
        assert!(!W::Conflict.overlaps(w) && !w.overlaps(W::Conflict), "{w:?}");
    }
}

#[test]
fn the_conflict_keys_are_rebindable_diff_and_conflict_rows() {
    let rows: Vec<&Binding> = BINDINGS.iter().filter(|b| b.when == W::Conflict).collect();
    let ids: Vec<BindingId> = rows.iter().map(|b| b.id).collect();
    let want = [
        I::ConflictTakeOurs,
        I::ConflictTakeTheirs,
        I::ConflictTakeBoth,
        I::ConflictTakeBothTheirsFirst,
        I::ConflictClear,
        I::ConflictMarkResolved,
    ];
    assert_eq!(ids, want);
    assert!(rows.iter().all(|b| b.rebindable && b.group == Group::Diff));
    assert_eq!(W::Conflict.note(), Some("in the conflict view"));
    assert_eq!(binding(I::RebaseContinue).label, "Continue the merge, rebase or cherry-pick");
    // A graph key can share a conflict key: Delete drops commits there, Backspace clears here.
    let keys = Keymap::default();
    assert_eq!(check(&keys, I::SelectHead, Chord::plain(KeyName::Letter('O')), fold_ctrl()), Check::Ok);
    assert_eq!(keys.lookup(Chord::plain(KeyName::Backspace), fold_ctrl()), Some(I::ConflictClear));
}
