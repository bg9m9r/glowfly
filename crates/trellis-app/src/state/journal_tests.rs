use super::*;
use crate::state::op_fixture::snapshot;
use trellis_core::journal::NotedRef;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn row(subject: Subject, n: u8, event: Event, message: &str) -> Row {
    Row {
        subject,
        oid: oid(n),
        prev: None,
        at: 1_700_000_000,
        event,
        message: message.into(),
        note: None,
        summary: Some("A commit".into()),
        current: false,
    }
}

fn note(op: Op, branch: &str) -> Note {
    Note { v: 1, at: 1_700_000_000, op, refs: vec![NotedRef { branch: Some(branch.into()), oid: oid(1).to_string() }] }
}

/// A snapshot whose `main` sits at `oid(tip)`.
fn snap_at(name: &str, tip: u8) -> Snapshot {
    let mut s = snapshot(name);
    let main = s.locals.iter_mut().find(|b| b.is_head).expect("the checked-out branch");
    main.name = "main".into();
    main.oid = Some(oid(tip));
    s.head_branch = Some("main".into());
    s.head_oid = Some(oid(tip));
    s
}

#[test]
fn a_label_prefers_the_note_then_the_event_then_the_raw_message() {
    let en = Lang::En;
    let mut r = row(Subject::Branch("main".into()), 1, Event::Reset { to: "1a2b3c4".into() }, "reset: moving to 1a2b3c4");
    assert_eq!(row_label(en, &r), "Reset to 1a2b3c4");
    r.note = Some(note(Op::Rebase { onto: Some("main".into()) }, "feature/login"));
    assert_eq!(row_label(en, &r), "Before rebasing feature/login onto main");
    let other = row(Subject::Head, 1, Event::Other, "something\tgit\nwrote");
    assert_eq!(row_label(en, &other), "something git wrote", "git's raw text, printable");
}

#[test]
fn labels_follow_the_language() {
    let de = Lang::De;
    let r = row(Subject::Head, 1, Event::Checkout { from: "main".into(), to: "feature".into() }, "checkout: moving from main to feature");
    assert_eq!(row_label(de, &r), "Checkout von feature");
    let n = note(Op::DeleteBranch { name: "old".into() }, "old");
    assert_eq!(note_label(de, &n), "Vor dem Löschen des Branches old");
    assert_eq!(event_label(de, &Event::Other), None);
    assert_eq!(event_label(de, &Event::RebaseStep), None);
}

#[test]
fn every_note_kind_names_its_branch() {
    let en = Lang::En;
    let cases = [
        (Op::Reset { mode: "hard".into(), to: "1a2b3c4".into() }, "Before resetting main to 1a2b3c4"),
        (Op::Restore { branch: Some("main".into()), to: "1a2b3c4".into() }, "Before restoring main to 1a2b3c4"),
        (Op::Undo { what: "hard reset to 1a2b3c4".into() }, "Before undoing hard reset to 1a2b3c4"),
        (Op::Merge { from: Some("topic".into()) }, "Before merging topic into main"),
        (Op::Amend, "Before amending the last commit on main"),
    ];
    for (op, want) in cases {
        assert_eq!(note_label(en, &note(op, "main")), want);
    }
    let detached = Note { refs: vec![NotedRef { branch: None, oid: oid(1).to_string() }], ..note(Op::Rewrite, "x") };
    assert_eq!(note_label(en, &detached), "Before rewriting the history of HEAD");
}

#[test]
fn the_moves_sentence_has_the_counts_and_says_when_they_are_capped() {
    let en = Lang::En;
    let snap = snap_at("journal-moves", 9);
    let r = row(Subject::Branch("main".into()), 1, Event::Commit { subject: "x".into() }, "commit: x");
    let short = |n: u8| oid(n).to_string()[..7].to_owned();
    let base = format!("Restore moves main from {} to {}", short(9), short(1));
    assert_eq!(moves_sentence(en, &r, None, &snap), Some(base.clone()), "before the preview loads");
    let p = |leaving: Count, arriving: Count| Preview { history: Vec::new(), leaving: Some(leaving), arriving: Some(arriving) };
    let n = |n: usize| Count { n, more: false };
    assert_eq!(
        moves_sentence(en, &r, Some(&p(n(3), n(1))), &snap),
        Some(format!("{base}: 3 commits leave the branch, 1 commit comes back"))
    );
    assert_eq!(moves_sentence(en, &r, Some(&p(n(0), n(2))), &snap), Some(format!("{base}: 2 commits come back")));
    let capped = Count { n: 1000, more: true };
    assert_eq!(moves_sentence(en, &r, Some(&p(capped, n(0))), &snap), Some(format!("{base}: 1,000+ commits leave the branch")));
    let here = row(Subject::Branch("main".into()), 9, Event::Commit { subject: "x".into() }, "commit: x");
    assert_eq!(moves_sentence(en, &here, None, &snap), None, "already there");
    let gone = row(Subject::Branch("old".into()), 1, Event::Other, "");
    assert_eq!(moves_sentence(en, &gone, None, &snap), Some(format!("Restore recreates old at {}", short(1))));
}

#[test]
fn against_is_the_branch_tip_or_head() {
    let snap = snap_at("journal-against", 4);
    assert_eq!(against(&snap, &Subject::Branch("main".into())), Some(oid(4)));
    assert_eq!(against(&snap, &Subject::Head), Some(oid(4)));
    assert_eq!(against(&snap, &Subject::Branch("gone".into())), None);
    assert_eq!(capped_line(Lang::En), "Showing the newest 2,000 states");
}
