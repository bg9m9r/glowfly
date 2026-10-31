//! Parking a `RepoSession` and resuming it after another tab's: the views, the open diff, the
//! draft, the selection, forge data, undo, search and view state move, the counters stay behind,
//! and every counter rises past both tabs'.

use super::*;
use crate::state::file_folds::{FileFolds, FileListId};
use crate::state::lfs::LfsDiffNote;
use crate::state::{ForgeState, Selection};
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

/// An undo entry (what it undoes doesn't matter here).
fn an_action() -> glowfly_core::undo::Action {
    glowfly_core::undo::Action::DeleteBranch { name: "old".into(), oid: git2::Oid::zero(), upstream: None }
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
    active.selection = Selection::Wip;
    active.forge = Some(ForgeState::new(None, "origin".into(), None));
    active.undo.record(an_action());
    active.search.filter = true;
    let v = &mut active.tab_view;
    (v.search, v.ref_filter, v.graph_scroll, v.diff_top_row) = ("fix".into(), "main".into(), (120.0, 400.0), 7);
    v.file_folds.toggle(FileListId::Unstaged, "src".into());
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

    assert_eq!(parked.selection, Selection::Wip, "the selection goes with the tab");
    assert!(parked.forge.is_some() && parked.undo.undo.len() == 1 && parked.search.filter, "and the forge data, undo and search");
    let v = &parked.tab_view;
    assert_eq!((v.search.as_str(), v.ref_filter.as_str(), v.graph_scroll, v.diff_top_row), ("fix", "main", (120.0, 400.0), 7));
    assert!(v.file_folds.is_closed(FileListId::Unstaged, "src"), "and the view state");
    assert_eq!(active.selection, Selection::None);
    assert!(active.forge.is_none() && active.undo.undo.is_empty() && !active.search.filter, "the next tab gets none of them");
    let v = &active.tab_view;
    assert!(v.search.is_empty() && v.ref_filter.is_empty() && v.graph_scroll == (0.0, 0.0) && v.diff_top_row == 0);
    assert_eq!(v.file_folds, FileFolds::default());
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
    let diff = DiffSlot { view: a_diff(), generation: 5, shown_gen: 5, lfs: Some(a_note()), raw: None };
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
    let mut parked =
        RepoSession { diff: DiffSlot { view: a_diff(), generation: 3, shown_gen: 3, lfs: None, raw: None }, ..Default::default() };
    assert_eq!(parked.diff.hunk_ops_gen(), Some(3));
    let mut current = RepoSession { diff: DiffSlot { generation: 9, shown_gen: 8, ..Default::default() }, ..Default::default() };

    parked.resume_after(&mut current);
    assert_eq!(parked.diff.generation, 10, "past both tabs' requests");
    assert_eq!(parked.diff.shown_gen, 8, "still the last request answered, not a counter");
    assert_eq!(parked.diff.hunk_ops_gen(), None, "the hunks wait for the reload");
}

#[test]
fn resume_drops_the_forge_replies_in_flight() {
    let mut parked = RepoSession { forge: Some(ForgeState::new(None, "origin".into(), None)), ..Default::default() };
    parked.forge.as_mut().unwrap().loading = 2;
    parked.resume_after(&mut RepoSession::default());
    assert_eq!(parked.forge.as_ref().map(|f| f.loading), Some(0), "the replies in flight were dropped while parked");
}

#[test]
fn forget_repo_clears_reads_and_folds() {
    let mut s = RepoSession::default();
    s.reads.stats.insert(git2::Oid::zero(), (1, 2, 3));
    s.tab_view.file_folds.toggle(FileListId::Staged, "docs".into());
    s.draft.summary = "wip".into();
    s.forget_repo();
    assert!(s.reads.stats.is_empty(), "the reads are forgotten");
    assert_eq!(s.tab_view.file_folds, FileFolds::default(), "and the folds");
    assert_eq!(s.draft.summary, "wip", "the rest is untouched");
}
