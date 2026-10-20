//! Building rows from reflog entries and notes, without a repository.

use super::events::Event;
use super::notes::{Note, NotedRef, Op};
use super::reflog::RawEntry;
use super::timeline::build;
use super::{MAX_ROWS, Row, Subject};
use git2::Oid;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn branch(name: &str) -> Subject {
    Subject::Branch(name.into())
}

fn entry(subject: Subject, new: u8, at: i64, message: &str, index: usize) -> RawEntry {
    RawEntry { subject, old: None, new: oid(new), at, message: message.into(), index }
}

fn note(at: i64, branch: Option<&str>, o: u8, op: Op) -> Note {
    Note { v: 1, at, op, refs: vec![NotedRef { branch: branch.map(Into::into), oid: oid(o).to_string() }] }
}

fn shape(rows: &[Row]) -> Vec<(Subject, i64, bool)> {
    rows.iter().map(|r| (r.subject.clone(), r.at, r.note.is_some())).collect()
}

#[test]
fn a_head_entry_folds_into_the_branch_entry_for_the_same_move() {
    let entries = vec![
        entry(Subject::Head, 2, 101, "commit: two", 0),
        entry(branch("main"), 2, 100, "commit: two", 0),
        entry(Subject::Head, 3, 203, "commit: three", 1),
        entry(branch("main"), 3, 200, "commit: three", 1),
    ];
    let (rows, capped) = build(entries, vec![], Some(oid(2)), &[("main".into(), oid(2))]);
    assert!(!capped);
    // 1 s apart folds; 3 s apart doesn't.
    assert_eq!(shape(&rows), vec![(Subject::Head, 203, false), (branch("main"), 200, false), (branch("main"), 100, false)]);
}

#[test]
fn a_checkout_or_a_branch_only_move_never_folds() {
    let entries = vec![
        entry(branch("topic"), 2, 100, "branch: Created from main", 0),
        entry(Subject::Head, 2, 101, "checkout: moving from main to topic", 0),
        entry(branch("x"), 3, 200, "branch: Created from HEAD", 0),
        entry(Subject::Head, 3, 200, "commit: three", 1),
    ];
    let (rows, _) = build(entries, vec![], None, &[]);
    assert_eq!(rows.len(), 4, "each move keeps its own row: {rows:#?}");
}

#[test]
fn rebase_steps_are_dropped_and_its_finish_kept() {
    let entries = vec![
        entry(branch("x"), 9, 50, "rebase (finish): refs/heads/x onto 0101010101010101010101010101010101010101", 0),
        entry(Subject::Head, 8, 49, "rebase (pick): two", 0),
        entry(Subject::Head, 7, 48, "rebase (start): checkout main", 1),
        entry(Subject::Head, 6, 10, "checkout: moving from main to x", 2),
    ];
    let (rows, _) = build(entries, vec![], None, &[]);
    let events: Vec<&Event> = rows.iter().map(|r| &r.event).collect();
    assert_eq!(
        events,
        vec![&Event::RebaseFinish { onto: Some("0101010".into()) }, &Event::Checkout { from: "main".into(), to: "x".into() }]
    );
}

#[test]
fn a_note_attaches_to_the_newest_matching_row_at_or_before_it() {
    let entries = vec![
        entry(branch("main"), 1, 300, "reset: moving to HEAD~1", 0),
        entry(branch("main"), 2, 200, "commit: two", 1),
        entry(branch("main"), 1, 100, "commit (initial): one", 2),
        entry(branch("other"), 2, 150, "branch: Created from main", 0),
    ];
    let reset = Op::Reset { mode: "hard".into(), to: "0101010".into() };
    let (rows, _) = build(entries, vec![note(250, Some("main"), 2, reset.clone())], None, &[]);
    let noted: Vec<_> = rows.iter().filter(|r| r.note.is_some()).map(|r| (r.subject.clone(), r.at)).collect();
    assert_eq!(noted, vec![(branch("main"), 200)], "the same subject and commit, not after the note");
    // The newest of two matching rows; one after the note doesn't count.
    let entries = vec![entry(branch("main"), 1, 300, "reset: moving to x", 0), entry(branch("main"), 1, 100, "commit (initial): one", 1)];
    let (rows, _) = build(entries, vec![note(200, Some("main"), 1, reset)], None, &[]);
    assert_eq!(shape(&rows), vec![(branch("main"), 300, false), (branch("main"), 100, true)]);
}

#[test]
fn notes_without_a_free_row_become_rows_of_their_own() {
    let entries = vec![entry(branch("main"), 1, 100, "commit (initial): one", 0)];
    let deleted = note(400, Some("old"), 5, Op::DeleteBranch { name: "old".into() });
    let amend = |at| note(at, Some("main"), 1, Op::Amend);
    let (rows, _) = build(entries, vec![amend(200), amend(300), deleted], None, &[]);
    assert_eq!(shape(&rows), vec![(branch("old"), 400, true), (branch("main"), 300, true), (branch("main"), 100, true)]);
    assert_eq!(rows[0].oid, oid(5));
    assert_eq!((&rows[0].event, rows[0].message.as_str()), (&Event::Other, ""));
}

#[test]
fn rows_are_newest_first_then_head_then_by_name_then_reflog_order() {
    let entries = vec![
        entry(branch("b"), 1, 100, "commit: x", 0),
        entry(branch("a"), 2, 100, "commit: x", 1),
        entry(branch("a"), 3, 100, "commit: y", 0),
        entry(Subject::Head, 4, 100, "checkout: moving from a to b", 0),
        entry(branch("a"), 5, 200, "commit: z", 2),
    ];
    let (rows, _) = build(entries, vec![], None, &[]);
    let order: Vec<(Subject, Oid)> = rows.iter().map(|r| (r.subject.clone(), r.oid)).collect();
    assert_eq!(
        order,
        vec![(branch("a"), oid(5)), (Subject::Head, oid(4)), (branch("a"), oid(3)), (branch("a"), oid(2)), (branch("b"), oid(1))]
    );
}

#[test]
fn more_rows_than_the_cap_are_cut() {
    let entries: Vec<RawEntry> = (0..MAX_ROWS + 5).map(|i| entry(branch("main"), 1 + (i % 200) as u8, i as i64, "commit: x", i)).collect();
    let (rows, capped) = build(entries, vec![], None, &[]);
    assert!(capped);
    assert_eq!(rows.len(), MAX_ROWS);
    assert_eq!(rows[0].at, (MAX_ROWS + 4) as i64, "the newest are kept");
}

#[test]
fn current_marks_only_each_subjects_newest_row_at_its_tip() {
    let entries = vec![
        entry(branch("main"), 2, 300, "commit: two", 0),
        entry(branch("main"), 1, 100, "commit (initial): one", 1),
        entry(branch("old"), 7, 250, "commit: moved on", 0),
        entry(branch("old"), 1, 50, "branch: Created from main", 1),
        entry(Subject::Head, 1, 200, "checkout: moving from old to main", 0),
    ];
    let tips = [("main".to_owned(), oid(2)), ("old".to_owned(), oid(1))];
    let (rows, _) = build(entries, vec![], Some(oid(1)), &tips);
    let current: Vec<(Subject, i64)> = rows.iter().filter(|r| r.current).map(|r| (r.subject.clone(), r.at)).collect();
    // old's newest row isn't at its tip (it was reset outside the reflog), so none of its rows is.
    assert_eq!(current, vec![(branch("main"), 300), (Subject::Head, 200)]);
}

#[test]
fn zero_new_ids_are_skipped() {
    let mut gone = entry(branch("main"), 0, 100, "commit: x", 0);
    gone.new = Oid::zero();
    let (rows, _) = build(vec![gone, entry(branch("main"), 1, 50, "commit: y", 1)], vec![], None, &[]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].oid, oid(1));
}
