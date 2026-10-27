//! Parking a `RepoSession` and resuming it after another tab's: the views, the open diff and
//! the draft move, the counters stay behind, and every counter rises past both tabs'.

use super::*;
use crate::state::lfs::LfsDiffNote;
use glowfly_core::lfs::pointer::{LfsChange, LfsSide};

/// Every counter, in the order `counters` lists them.
fn counts(s: &mut RepoSession) -> Vec<u64> {
    s.counters().into_iter().map(|c| *c).collect()
}

/// An empty diff of `a.txt`'s working copy.
fn a_diff() -> Option<(DiffTarget, std::sync::Arc<FileDiff>)> {
    let d = FileDiff { binary: false, hunks: vec![], highlight: None, intraline: Default::default(), whole: None };
    Some((DiffTarget::Unstaged("a.txt".into()), d.into()))
}

fn a_note() -> LfsDiffNote {
    let change = LfsChange { old: LfsSide::Absent, new: LfsSide::Regular { size: 4 }, new_in_workdir: true, filtered: false };
    LfsDiffNote { target: DiffTarget::Unstaged("a.txt".into()), change, show_pointer: true }
}

/// Sets every counter to `n`.
fn set_all(s: &mut RepoSession, n: u64) {
    for c in s.counters() {
        *c = n;
    }
}

#[test]
fn park_empties_the_views_and_keeps_the_counters() {
    let mut active = RepoSession {
        history: Gen { view: Some(FileHistoryView::loading("a.txt".into(), None)), generation: 4 },
        blame: Gen { view: Some(BlameView::loading("a.txt".into(), None)), generation: 2 },
        journal: Gen::open(JournalView::default()),
        gitflow_panel: true,
        ..Default::default()
    };
    active.pull_request.page_gen = 9;
    active.pull_request.lines_gen = 3;
    active.range_diff.generation = 6;
    let before = counts(&mut active);

    let mut parked = active.park();
    assert!(parked.history.view.is_some() && parked.blame.view.is_some() && parked.journal.view.is_some(), "the views go with the tab");
    assert!(parked.gitflow_panel);
    let after = counts(&mut parked);
    assert_eq!(after[..12], before[..12], "the parked session keeps the views' counters");
    let reads: Vec<u64> = before[12..].iter().map(|c| c + 1).collect();
    assert_eq!(after[12..], reads[..], "the reads' counters move past their requests in flight");
    assert_eq!(parked.history.generation, 4);
    assert_eq!(parked.pull_request.page_gen, 9);

    assert!(active.history.view.is_none() && active.blame.view.is_none() && active.journal.view.is_none());
    assert!(active.conflict.view.is_none() && active.pull_request.view.is_none() && active.rebase.view.is_none());
    assert!(!active.gitflow_panel, "the panel isn't left open for the next tab");
    assert_eq!(counts(&mut active), after, "the counters stay behind as well");
}

#[test]
fn resume_moves_every_counter_past_both() {
    let (mut parked, mut current) = (RepoSession::default(), RepoSession::default());
    set_all(&mut parked, 3);
    set_all(&mut current, 7);
    parked.resume_after(&mut current);
    assert_eq!(counts(&mut parked), vec![8; 15]);
    let pr = &parked.pull_request;
    assert_eq!((pr.generation, pr.page_gen, pr.checks_gen, pr.lines_gen), (8, 8, 8, 8), "the panel's four counters are all in the list");
    assert_eq!((parked.conflict.generation, parked.issue.generation, parked.range_diff.generation), (8, 8, 8));
    assert_eq!(parked.diff.generation, 8, "the diff's counter is in the list");
    let reads: Vec<u64> = parked.reads.counters().into_iter().map(|c| *c).collect();
    assert_eq!(reads, vec![8; 3], "the reads' three counters are in the list");

    // Can't happen (the active counters are never behind a parked tab's), but stays safe.
    set_all(&mut parked, 10);
    set_all(&mut current, 2);
    parked.resume_after(&mut current);
    assert_eq!(counts(&mut parked), vec![11; 15]);
}

#[test]
fn parking_drops_a_conflict_save_in_flight() {
    let mut active =
        RepoSession { conflict: Gen::open(ConflictView { saving: true, ..ConflictView::loading("a.txt".into()) }), ..Default::default() };
    let parked = active.park();
    assert_eq!(parked.conflict.view.as_ref().map(|c| c.saving), Some(false));
}

#[test]
fn park_takes_the_open_diff_and_leaves_its_counters() {
    let diff = DiffSlot { view: a_diff(), generation: 5, shown_gen: 5, lfs: Some(a_note()) };
    let mut active = RepoSession { diff, ..Default::default() };
    active.draft.summary = "wip".into();

    let parked = active.park();
    assert!(parked.diff.view.is_some() && parked.diff.lfs == Some(a_note()), "the diff and its note go with the tab");
    assert_eq!(parked.draft.summary, "wip");
    assert_eq!((parked.diff.generation, parked.diff.shown_gen), (5, 5));

    assert!(active.diff.view.is_none() && active.diff.lfs.is_none());
    assert!(active.draft.summary.is_empty(), "the next tab doesn't get the draft");
    assert_eq!((active.diff.generation, active.diff.shown_gen), (5, 5), "both stay behind");
}

#[test]
fn resume_moves_the_diff_counter_and_keeps_the_shown_request_current() {
    let mut parked = RepoSession { diff: DiffSlot { view: a_diff(), generation: 3, shown_gen: 3, lfs: None }, ..Default::default() };
    assert_eq!(parked.diff.hunk_ops_gen(), Some(3));
    let mut current = RepoSession { diff: DiffSlot { generation: 9, shown_gen: 8, ..Default::default() }, ..Default::default() };

    parked.resume_after(&mut current);
    assert_eq!(parked.diff.generation, 10, "past both tabs' requests");
    assert_eq!(parked.diff.shown_gen, 8, "still the last request answered, not a counter");
    assert_eq!(parked.diff.hunk_ops_gen(), None, "the hunks wait for the reload");
}
