//! The pane keys (Alt+1 to 5, Control+1 to 5 on macOS, F6 and Shift+F6) and the graph's keys:
//! their rows and defaults, that they clash with no other key (the menus' Alt+letter mnemonics
//! included), and that the graph's keys may be a diff's too.

use super::rebind::Refusal;
use super::*;

use BindingId as I;
use KeyName as K;

const PANES: [BindingId; 5] = [I::FocusSidebar, I::FocusCentral, I::FocusFiles, I::FocusDiff, I::FocusCommitMessage];

#[test]
fn pane_keys_are_rebindable_navigation_rows_that_work_anywhere() {
    let keys = Keymap::default();
    let fold = fold_ctrl();
    for (n, id) in PANES.into_iter().enumerate() {
        let b = binding(id);
        assert_eq!((b.group, b.when, b.rebindable), (Group::Navigation, When::Anywhere, true), "{id:?}");
        let digit = K::Digit(n as u8 + 1);
        let want = if cfg!(target_os = "macos") { Chord::ctrl(digit) } else { Chord::alt(digit) };
        assert_eq!(b.defaults, [want], "{id:?}");
        assert_eq!(keys.lookup(want, fold), Some(id), "{id:?}");
    }
    for (id, want) in [(I::NextPane, Chord::plain(K::F(6))), (I::PrevPane, Chord::shift(K::F(6)))] {
        let b = binding(id);
        assert_eq!((b.group, b.when, b.rebindable, b.defaults), (Group::Navigation, When::Anywhere, true, &[want][..]), "{id:?}");
    }
    // Go to the commit message keeps its own key.
    assert_eq!(binding(I::FocusMessage).defaults, [Chord::cmd_shift(K::Letter('M'))]);
}

#[test]
fn pane_keys_leave_the_alt_keys_and_menu_mnemonics_alone() {
    let fold = fold_ctrl();
    let own = |id: BindingId| PANES.contains(&id) || matches!(id, I::NextPane | I::PrevPane);
    let pane_chords: Vec<Chord> = BINDINGS.iter().filter(|b| own(b.id)).flat_map(|b| b.defaults.to_vec()).collect();
    // No other binding has a pane key, wherever it works.
    for b in BINDINGS.iter().filter(|b| !own(b.id)) {
        for d in b.defaults {
            assert!(!pane_chords.iter().any(|p| super::active::answers(*d, *p, fold)), "{:?} has a pane key {d:?}", b.id);
        }
    }
    // The menus' mnemonics are Alt+letters (File, Edit, View, Window, Help): the pane keys are
    // digits and F6, and no binding takes those letters with Alt alone.
    for c in ['F', 'E', 'V', 'W', 'H'] {
        assert_eq!(Keymap::default().lookup(Chord::alt(K::Letter(c)), fold), None, "Alt+{c}");
    }
    assert!(pane_chords.iter().all(|c| matches!(c.key, K::Digit(_) | K::F(6))));
}

#[test]
fn graph_keys() {
    let keys = Keymap::default();
    let fold = fold_ctrl();
    for (id, c) in [
        (I::SelectHead, Chord::plain(K::Letter('H'))),
        (I::PrevRefCommit, Chord::plain(K::OpenBracket)),
        (I::NextRefCommit, Chord::plain(K::CloseBracket)),
        (I::CopyCommitIds, Chord::cmd(K::Letter('C'))),
    ] {
        assert_eq!((binding(id).when, binding(id).rebindable), (When::Graph, true), "{id:?}");
        assert_eq!(keys.lookup(c, fold), Some(id), "{id:?}");
        assert_eq!(check(&keys, id, c, fold), Check::Same, "{id:?}");
    }
    // Enter, Page Up/Down and Home/End are fixed: the graph matches them itself.
    for id in [I::OpenCommitDetails, I::GraphPageRow, I::GraphEdgeRow] {
        assert!(!binding(id).rebindable, "{id:?}");
    }
    assert_eq!(binding(I::OpenCommitDetails).when, When::Graph);
    assert_eq!((binding(I::GraphPageRow).when, binding(I::GraphEdgeRow).when), (When::Lists, When::Lists));
    // Cherry-pick and Revert are Repository commands rows with no key.
    for id in [I::CherryPick, I::RevertCommit] {
        assert!(runs_command(id) && binding(id).defaults.is_empty() && binding(id).group == Group::Commands, "{id:?}");
    }
}

#[test]
fn the_graph_is_never_shown_with_a_diff() {
    for other in [When::WorkingDiff, When::CommitDiff, When::DiffOrConflict, When::Diff, When::Blame] {
        assert!(!When::Graph.overlaps(other) && !other.overlaps(When::Graph), "{other:?}");
    }
    for other in [When::Graph, When::Lists, When::NotTyping, When::Anywhere] {
        assert!(When::Graph.overlaps(other) && other.overlaps(When::Graph), "{other:?}");
    }
    let keys = Keymap::default();
    let fold = fold_ctrl();
    let h = Chord::plain(K::Letter('H'));
    // A diff's key may be a graph key (H selects HEAD in the graph); a key of the graph and
    // file history (Lists) may not.
    assert_eq!(check(&keys, I::OpenFold, h, fold), Check::Ok);
    assert_eq!(check(&keys, I::SelectAllCommits, h, fold), Check::Clash { other: I::SelectHead, fixed: false });
    // Ctrl+C stays copy everywhere else.
    assert_eq!(check(&keys, I::RevertHunk, Chord::cmd(K::Letter('C')), fold), Check::Refused(Refusal::ClipboardKey));
}
