//! Parking a `RepoSession` and resuming it after another tab's: the views move, the counters
//! stay behind, and every counter rises past both tabs'.

use super::*;

/// Every counter, in the order `counters` lists them.
fn counts(s: &mut RepoSession) -> Vec<u64> {
    s.counters().into_iter().map(|c| *c).collect()
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
    assert_eq!(counts(&mut parked), before, "the parked session keeps its counters");
    assert_eq!(parked.history.generation, 4);
    assert_eq!(parked.pull_request.page_gen, 9);

    assert!(active.history.view.is_none() && active.blame.view.is_none() && active.journal.view.is_none());
    assert!(active.conflict.view.is_none() && active.pull_request.view.is_none() && active.rebase.view.is_none());
    assert!(!active.gitflow_panel, "the panel isn't left open for the next tab");
    assert_eq!(counts(&mut active), before, "the counters stay behind as well");
}

#[test]
fn resume_moves_every_counter_past_both() {
    let (mut parked, mut current) = (RepoSession::default(), RepoSession::default());
    set_all(&mut parked, 3);
    set_all(&mut current, 7);
    parked.resume_after(&mut current);
    assert_eq!(counts(&mut parked), vec![8; 11]);
    let pr = &parked.pull_request;
    assert_eq!((pr.generation, pr.page_gen, pr.checks_gen, pr.lines_gen), (8, 8, 8, 8), "the panel's four counters are all in the list");
    assert_eq!((parked.conflict.generation, parked.issue.generation, parked.range_diff.generation), (8, 8, 8));

    // Can't happen (the active counters are never behind a parked tab's), but stays safe.
    set_all(&mut parked, 10);
    set_all(&mut current, 2);
    parked.resume_after(&mut current);
    assert_eq!(counts(&mut parked), vec![11; 11]);
}

#[test]
fn parking_drops_a_conflict_save_in_flight() {
    let mut active =
        RepoSession { conflict: Gen::open(ConflictView { saving: true, ..ConflictView::loading("a.txt".into()) }), ..Default::default() };
    let parked = active.park();
    assert_eq!(parked.conflict.view.as_ref().map(|c| c.saving), Some(false));
}
