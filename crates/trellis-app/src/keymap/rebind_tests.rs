//! The rebinding rules: which places overlap, folding Ctrl into Mod, which keys are refused,
//! clashes with fixed and rebindable bindings, Swap, Add and Reset.

use super::rebind::Refusal;
use super::*;

use BindingId as I;
use KeyName as K;

const WHENS: [When; 14] = [
    When::Anywhere,
    When::NotTyping,
    When::WorkingDiff,
    When::CommitDiff,
    When::DiffOrConflict,
    When::Message,
    When::Lists,
    When::Search,
    When::InPalette,
    When::PaletteStep,
    When::Dialog,
    When::Blame,
    When::Graph,
    When::RebasePanel,
];

#[test]
fn overlaps_is_symmetric() {
    for a in WHENS {
        assert!(a.overlaps(a), "{a:?}");
        for b in WHENS {
            assert_eq!(a.overlaps(b), b.overlaps(a), "{a:?} {b:?}");
        }
    }
    assert!(When::Anywhere.overlaps(When::Message));
    assert!(!When::NotTyping.overlaps(When::Message));
    assert!(When::WorkingDiff.overlaps(When::DiffOrConflict));
    assert!(When::CommitDiff.overlaps(When::NotTyping) && When::CommitDiff.overlaps(When::WorkingDiff));
    assert!(When::CommitDiff.overlaps(When::DiffOrConflict) && !When::CommitDiff.overlaps(When::Message));
    assert!(When::InPalette.overlaps(When::PaletteStep));
    assert!(!When::Search.overlaps(When::Dialog));
    assert!(!When::Lists.overlaps(When::InPalette));
    assert!(When::Blame.overlaps(When::NotTyping) && !When::Blame.overlaps(When::Search));
}

#[test]
fn folding_ctrl() {
    let (pc, mac) = (Chord::ctrl(K::Tab), Chord::cmd(K::Tab));
    assert!(same_keys(pc, mac, true));
    assert!(!same_keys(pc, mac, false));
    assert!(!same_keys(Chord::cmd(K::Tab), Chord::cmd_shift(K::Tab), true));
    assert!(!same_keys(Chord::plain(K::Up), Chord::alt(K::Up), true));
}

fn keys() -> Keymap {
    Keymap::default()
}

#[test]
fn check_same_and_refused() {
    let k = keys();
    assert_eq!(check(&k, I::NewTab, Chord::cmd(K::Letter('T')), true), Check::Same);
    for c in [
        Chord::plain(K::Enter),
        Chord::shift(K::Tab),
        Chord::plain(K::Escape),
        Chord::cmd(K::Escape),
        Chord::plain(K::Space),
        Chord::plain(K::Up),
    ] {
        assert_eq!(check(&k, I::Refresh, c, true), Check::Refused(Refusal::Reserved), "{c:?}");
    }
    assert_eq!(check(&k, I::Refresh, Chord::cmd(K::Letter('C')), true), Check::Refused(Refusal::ClipboardKey));
    assert_eq!(check(&k, I::Undo, Chord::cmd(K::Letter('V')), true), Check::Refused(Refusal::ClipboardKey));
    assert_eq!(check(&k, I::Palette, Chord::plain(K::Letter('P')), true), Check::Refused(Refusal::NeedsModifier));
    assert_eq!(check(&k, I::Palette, Chord::shift(K::Comma), true), Check::Refused(Refusal::NeedsModifier));
    assert_eq!(check(&k, I::Undo, Chord::plain(K::Letter('Z')), true), Check::Ok, "NotTyping takes plain keys");
    assert_eq!(check(&k, I::Refresh, Chord::plain(K::F(9)), true), Check::Ok, "F-keys need no modifier");
    assert_eq!(check(&k, I::Refresh, Chord::plain(K::Insert), true), Check::Ok);
    // Delete edits text, so an action that works while typing can't take it plain.
    for c in [Chord::plain(K::Delete), Chord::shift(K::Delete)] {
        assert_eq!(check(&k, I::CloseTab, c, true), Check::Refused(Refusal::NeedsModifier), "{c:?}");
    }
    assert_eq!(check(&k, I::CloseTab, Chord::cmd(K::Delete), true), Check::Ok);
    assert_eq!(check(&k, I::Undo, Chord::shift(K::Delete), true), Check::Ok, "NotTyping takes Delete without Ctrl or Alt");
    let delete = check(&k, I::Undo, Chord::plain(K::Delete), true);
    assert_eq!(delete, Check::Clash { other: I::RebaseRemoveStep, fixed: true }, "the rebase panel's Delete");
}

#[test]
fn check_clashes() {
    let k = keys();
    assert_eq!(check(&k, I::Refresh, Chord::cmd(K::Digit(3)), true), Check::Clash { other: I::GoToTab, fixed: true });
    assert_eq!(check(&k, I::Palette, Chord::cmd(K::Enter), true), Check::Clash { other: I::Commit, fixed: true });
    assert_eq!(check(&k, I::Undo, Chord::cmd(K::Enter), true), Check::Ok, "the commit message is a text field");
    assert_eq!(check(&k, I::Refresh, Chord::cmd(K::Letter('T')), true), Check::Clash { other: I::NewTab, fixed: false });
    assert_eq!(check(&k, I::Stage, Chord::cmd(K::Letter('Z')), true), Check::Clash { other: I::Undo, fixed: false });
}

#[test]
fn selecting_commits_keys() {
    let k = keys();
    // Mod+A stays select all: only Select every commit may have it.
    assert_eq!(check(&k, I::SelectAllCommits, Chord::cmd(K::Letter('A')), true), Check::Same);
    assert_eq!(check(&k, I::Refresh, Chord::cmd(K::Letter('A')), true), Check::Refused(Refusal::ClipboardKey));
    assert_eq!(check(&k, I::Stage, Chord::cmd(K::Letter('A')), true), Check::Refused(Refusal::ClipboardKey));
    // Lists overlap the diff and the rest of the browsing places.
    assert_eq!(check(&k, I::SelectAllCommits, Chord::cmd(K::Letter('Z')), true), Check::Clash { other: I::Undo, fixed: false });
    // Shift+arrows are kept for moving around, so nothing else can take them.
    assert_eq!(check(&k, I::NextChange, Chord::shift(K::Down), true), Check::Refused(Refusal::Reserved));
    // Alt+arrows aren't: they clash as usual (in the lists, with Next change).
    assert_eq!(check(&k, I::SelectAllCommits, Chord::alt(K::Down), true), Check::Clash { other: I::NextChange, fixed: false });
}

#[test]
fn swap() {
    let k = keys();
    assert_eq!(swap_chord(&k, I::Refresh, Slot::Replace(0), I::NewTab, true), Some(Chord::plain(K::F(5))));
    assert_eq!(swap_chord(&k, I::Refresh, Slot::Add, I::NewTab, true), None);
    assert_eq!(swap_chord(&k, I::Stage, Slot::Replace(0), I::Palette, true), None, "S can't open the palette while typing");
}

#[test]
fn apply_to_a_slot() {
    let k = keys();
    let f9 = Chord::plain(K::F(9));
    let (p, sp) = (Chord::cmd(K::Letter('P')), Chord::cmd_shift(K::Letter('P')));
    assert_eq!(apply(&k, I::Palette, Slot::Replace(1), f9), [p, f9]);
    assert_eq!(apply(&k, I::Palette, Slot::Add, f9), [p, sp, f9]);
}

#[test]
fn reset_takes_its_keys_back() {
    let mut k = keys();
    let t = Chord::cmd(K::Letter('T'));
    k.set(I::NewTab, Vec::new());
    k.set(I::Refresh, vec![t, Chord::plain(K::F(5))]);
    let taken = reset_taking(&mut k, I::NewTab, true);
    assert_eq!(taken, [(I::Refresh, t)]);
    assert_eq!(k.chords(I::NewTab), [t]);
    assert_eq!(k.chords(I::Refresh), [Chord::plain(K::F(5))]);
    assert!(reset_taking(&mut k, I::NewTab, true).is_empty());
}
