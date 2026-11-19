//! The conflict keys: what each sends for the open conflict view, Mark resolved only while it
//! is enabled (also for a file without conflicts, where the pick keys do nothing), and Continue
//! from the conflict strip or the rebase.

use super::*;
use crate::msg::CommitMsg;
use crate::state::conflict::ConflictView;
use crate::state::op_fixture::{rows, snapshot};
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::{Chunk, ConflictContent, ConflictFile, ConflictIds, Operation, Segment, SideLabels};
use glowfly_core::settings::Settings;

/// The open file f.txt with `segs` as its text.
fn file(segs: Vec<Segment>) -> ConflictFile {
    ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "theme-tweaks".into() },
        content: ConflictContent::Text(segs),
        no_ancestor: false,
        base_markers: false,
    }
}

/// A merge with `conflicted` files conflicted and the view open on f.txt, two chunks to pick.
fn st(name: &str, state: RepositoryState, conflicted: usize) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), std::time::Instant::now());
    let mut snap = snapshot(name);
    snap.state = state;
    let names: Vec<String> = (0..conflicted).map(|i| format!("c{i}.txt")).collect();
    snap.unstaged = rows(&names.iter().map(|n| (n.as_str(), '!')).collect::<Vec<_>>());
    st.snap = Some(snap);
    let line = |s: &str| vec![format!("{s}\n")];
    let chunk = |i: usize| Segment::Conflict(Chunk { base: line("b"), ours: line(&format!("o{i}")), theirs: line(&format!("t{i}")) });
    let file = file(vec![chunk(0), Segment::Clean(line("mid")), chunk(1)]);
    let mut view = ConflictView::loading("f.txt".into());
    view.set_file(file);
    st.session.conflict.view = Some(view);
    st
}

/// Like `st`, but the view holds a text file the stages merge cleanly: no conflict chunks.
fn chunkless(name: &str) -> AppState {
    let mut st = st(name, RepositoryState::Merge, 1);
    let mut view = ConflictView::loading("f.txt".into());
    view.set_file(file(vec![Segment::Clean(vec![
        "a
"
        .into(),
    ])]));
    st.session.conflict.view = Some(view);
    st
}

fn picked(m: Option<Msg>) -> Option<Choice> {
    match m {
        Some(Msg::Conflict(ConflictMsg::ConflictPickCurrent(c))) => Some(c),
        _ => None,
    }
}

#[test]
fn the_pick_keys_pick_whole_sides_for_the_current_conflict() {
    use BindingId as I;
    let s = st("keys-pick", RepositoryState::Merge, 1);
    assert_eq!(picked(conflict_key(&s, I::ConflictTakeOurs)), Some(Choice::Ours));
    assert_eq!(picked(conflict_key(&s, I::ConflictTakeTheirs)), Some(Choice::Theirs));
    assert_eq!(picked(conflict_key(&s, I::ConflictTakeBoth)), Some(Choice::OursThenTheirs));
    assert_eq!(picked(conflict_key(&s, I::ConflictTakeBothTheirsFirst)), Some(Choice::TheirsThenOurs));
    assert_eq!(picked(conflict_key(&s, I::ConflictClear)), Some(Choice::Unresolved));
    assert!(conflict_key(&s, I::Stage).is_none(), "not a conflict key");

    let mut s = s;
    s.session.conflict.view = Some(ConflictView::loading("f.txt".into()));
    assert!(conflict_key(&s, I::ConflictTakeOurs).is_none(), "still loading: no conflict to pick");
    s.session.conflict.view = None;
    assert!(conflict_key(&s, I::ConflictTakeOurs).is_none());
}

#[test]
fn mark_resolved_waits_until_every_conflict_is_picked() {
    let mut s = st("keys-mark", RepositoryState::Merge, 1);
    assert!(conflict_key(&s, BindingId::ConflictMarkResolved).is_none(), "disabled with two conflicts open");
    let v = s.session.conflict.view.as_mut().unwrap();
    v.picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs)];
    let m = conflict_key(&s, BindingId::ConflictMarkResolved);
    assert!(matches!(m, Some(Msg::Conflict(ConflictMsg::SaveConflict))));
    s.session.conflict.view.as_mut().unwrap().saving = true;
    assert!(conflict_key(&s, BindingId::ConflictMarkResolved).is_none(), "not while saving");
}

#[test]
fn mark_resolved_saves_a_file_without_conflicts() {
    let mut s = chunkless("keys-mark-chunkless");
    assert_eq!(s.session.conflict.view.as_ref().unwrap().chunk_count(), 0);
    assert!(matches!(conflict_key(&s, BindingId::ConflictMarkResolved), Some(Msg::Conflict(ConflictMsg::SaveConflict))));
    s.session.conflict.view.as_mut().unwrap().saving = true;
    assert!(conflict_key(&s, BindingId::ConflictMarkResolved).is_none(), "not while saving");
    s.session.conflict.view = Some(ConflictView::loading("f.txt".into()));
    assert!(conflict_key(&s, BindingId::ConflictMarkResolved).is_none(), "not while loading");
    s.session.conflict.view = None;
    assert!(conflict_key(&s, BindingId::ConflictMarkResolved).is_none(), "not without a view");
}

#[test]
fn the_pick_keys_need_a_conflict() {
    use BindingId as I;
    let s = chunkless("keys-pick-chunkless");
    for id in [I::ConflictTakeOurs, I::ConflictTakeTheirs, I::ConflictTakeBoth, I::ConflictTakeBothTheirsFirst, I::ConflictClear] {
        assert!(conflict_key(&s, id).is_none(), "{id:?}");
    }
}

#[test]
fn mark_resolved_is_enabled_for_one_predicate() {
    let s = chunkless("keys-enabled-chunkless");
    let v = s.session.conflict.view.as_ref().unwrap();
    assert!(v.mark_resolved_enabled(false));
    assert!(!v.mark_resolved_enabled(true), "the merge tool has the file open");
    let mut s = st("keys-enabled-two", RepositoryState::Merge, 1);
    assert!(!s.session.conflict.view.as_ref().unwrap().mark_resolved_enabled(false), "two conflicts open");
    s.session.conflict.view.as_mut().unwrap().picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs)];
    assert!(s.session.conflict.view.as_ref().unwrap().mark_resolved_enabled(false));
}

#[test]
fn continue_runs_the_strips_button_once_it_can() {
    let s = st("keys-continue-wait", RepositoryState::Merge, 1);
    assert!(continue_msg(&s).is_none(), "a merge waits for its conflicted files");
    let s = st("keys-continue-merge", RepositoryState::Merge, 0);
    assert!(matches!(conflict_key(&s, BindingId::RebaseContinue), Some(Msg::Commit(CommitMsg::CommitMerge))));
    let s = st("keys-continue-cherry", RepositoryState::CherryPick, 0);
    assert!(continue_msg(&s).is_some(), "a cherry-pick with nothing conflicted continues");
    // A rebase still waiting on files goes to the rebase's Continue, which says what stops it.
    let mut s = st("keys-continue-rebase", RepositoryState::RebaseMerge, 1);
    assert!(matches!(continue_msg(&s), Some(Msg::Rebase(RebaseMsg::Resume(Resume::Continue)))));
    s.session.conflict.view = None;
    assert!(matches!(continue_msg(&s), Some(Msg::Rebase(RebaseMsg::Resume(Resume::Continue)))), "without the view too");
}
