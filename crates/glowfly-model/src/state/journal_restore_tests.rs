use super::*;
use crate::state::op_fixture::{rows, snapshot};
use glowfly_core::journal::Count;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn row(subject: Subject, n: u8, event: Event) -> Row {
    Row {
        subject,
        oid: oid(n),
        prev: None,
        at: 1_700_000_000,
        event,
        message: String::new(),
        note: None,
        summary: Some("s".into()),
        current: false,
        unreachable: false,
    }
}

/// A snapshot with `main` checked out at `oid(9)` and `topic` (not checked out) at `oid(8)`.
fn snap(name: &str) -> Snapshot {
    let mut s = snapshot(name);
    let mut topic = s.locals[0].clone();
    let main = &mut s.locals[0];
    main.name = "main".into();
    main.oid = Some(oid(9));
    topic.name = "topic".into();
    topic.oid = Some(oid(8));
    topic.is_head = false;
    s.locals.push(topic);
    s.head_branch = Some("main".into());
    s.head_oid = Some(oid(9));
    s
}

fn branch(name: &str) -> Subject {
    Subject::Branch(name.into())
}

#[test]
fn each_subject_gets_its_own_restore() {
    let (s, en) = (snap("journal-plan"), Lang::En);
    let commit = Event::Commit { subject: "x".into() };
    assert_eq!(
        plan(&s, &row(branch("main"), 1, commit.clone()), false, en),
        Ok(RestorePlan::ResetHead { branch: "main".into(), from: oid(9), to: oid(1) })
    );
    assert_eq!(
        plan(&s, &row(branch("topic"), 1, commit.clone()), false, en),
        Ok(RestorePlan::MoveBranch { name: "topic".into(), from: oid(8), to: oid(1) })
    );
    assert_eq!(plan(&s, &row(branch("old"), 1, Event::Other), false, en), Ok(RestorePlan::CreateBranch { name: "old".into(), at: oid(1) }));
    let checkout = |to: &str| Event::Checkout { from: "main".into(), to: to.into() };
    assert_eq!(
        plan(&s, &row(Subject::Head, 8, checkout("topic")), false, en),
        Ok(RestorePlan::CheckoutBranch { name: "topic".into(), to: oid(8), from: Some(oid(9)) }),
        "topic still sits at the row's commit"
    );
    assert_eq!(
        plan(&s, &row(Subject::Head, 7, checkout("topic")), false, en),
        Ok(RestorePlan::CheckoutDetached { to: oid(7), from: Some(oid(9)) })
    );
    assert_eq!(plan(&s, &row(Subject::Head, 7, commit), false, en), Ok(RestorePlan::CheckoutDetached { to: oid(7), from: Some(oid(9)) }));
}

#[test]
fn every_reason_it_cannot_restore() {
    let (s, en) = (snap("journal-refuse"), Lang::En);
    let r = row(branch("topic"), 1, Event::Other);
    assert_eq!(plan(&s, &r, true, en), Err("Another operation is still running.".into()));
    let here = row(branch("topic"), 8, Event::Other);
    assert_eq!(plan(&s, &here, false, en), Err("topic is already at this commit.".into()));
    assert_eq!(plan(&s, &row(Subject::Head, 9, Event::Other), false, en), Err("HEAD is already at this commit.".into()));
    let gone = Row { summary: None, ..r.clone() };
    assert_eq!(plan(&s, &gone, false, en), Err("This commit is no longer in the repository.".into()));
    let mut merging = snap("journal-refuse-merge");
    merging.state = RepositoryState::Merge;
    assert_eq!(plan(&merging, &r, false, en), Err("Finish or abort the operation in progress first.".into()));
    let mut conflicted = snap("journal-refuse-conflict");
    conflicted.unstaged = rows(&[("a.txt", '!')]);
    assert_eq!(plan(&conflicted, &r, false, en), Err("Finish or abort the operation in progress first.".into()));
}

#[test]
fn the_confirmation_names_the_move_the_counts_and_undo() {
    let (s, en) = (snap("journal-confirm"), Lang::En);
    let p = plan(&s, &row(branch("main"), 1, Event::Other), false, en).unwrap();
    let preview = Preview { history: Vec::new(), leaving: Some(Count { n: 3, more: false }), arriving: Some(Count { n: 1, more: false }) };
    let Msg::Confirm { title, body, button, then } = confirm(en, &p, Some(&preview), false, Some("main")) else { panic!("a confirm") };
    assert_eq!(title, "Restore main?");
    let (a, b) = (&oid(9).to_string()[..7], &oid(1).to_string()[..7]);
    assert_eq!(
        body,
        format!(
            "main moves from {a} to {b}. 3 commits leave the branch; they stay in the journal. 1 commit comes back. The working tree is reset to match. Undo puts it back."
        )
    );
    assert_eq!(button, "Restore");
    let Msg::Job(JobMsg::Journaled { cmd, op, expect, .. }) = *then else { panic!("the restore job") };
    assert_eq!(cmd, restore::reset_to(oid(1)));
    assert_eq!(op, Op::Restore { branch: Some("main".into()), to: b.into() });
    assert_eq!(expect, Some(("refs/heads/main".into(), oid(9))));
}

#[test]
fn a_dirty_tree_stashes_in_the_same_dialog_for_a_reset_but_not_a_move() {
    let (s, en) = (snap("journal-dirty"), Lang::En);
    let reset = plan(&s, &row(branch("main"), 1, Event::Other), false, en).unwrap();
    let Msg::Confirm { body, button, then, .. } = confirm(en, &reset, None, true, Some("main")) else { panic!() };
    assert_eq!(button, "Stash and restore");
    assert!(body.contains("Your uncommitted changes are stashed first and put back afterwards."), "{body}");
    assert!(matches!(*then, Msg::AutoStash(AutoStashMsg::Retry(_))));
    let checkout = plan(&s, &row(Subject::Head, 7, Event::Other), false, en).unwrap();
    let Msg::Confirm { title, button, .. } = confirm(en, &checkout, None, true, Some("main")) else { panic!() };
    assert_eq!((title.as_str(), button.as_str()), ("Check out this state?", "Stash and check out"));
    let moved = plan(&s, &row(branch("topic"), 1, Event::Other), false, en).unwrap();
    let Msg::Confirm { body, button, then, .. } = confirm(en, &moved, None, true, Some("main")) else { panic!() };
    assert_eq!(button, "Restore");
    assert!(body.contains("Your working tree isn't touched."), "{body}");
    assert!(matches!(*then, Msg::Job(JobMsg::Journaled { .. })), "no stash for a branch that isn't checked out");
    let create = plan(&s, &row(branch("old"), 1, Event::Other), false, en).unwrap();
    let Msg::Confirm { title, button, .. } = confirm(en, &create, None, true, Some("main")) else { panic!() };
    assert_eq!((title.as_str(), button.as_str()), ("Recreate branch old?", "Create branch"));
}

#[test]
fn create_branch_here_names_a_deleted_branch() {
    let s = snap("journal-create");
    let start = Some(oid(1).to_string());
    let crate::msg::BranchMsg::NewBranch { start: at, name } = create_branch(&row(branch("old"), 1, Event::Other), &s) else { panic!() };
    assert_eq!((at, name.as_str()), (start.clone(), "old"));
    let crate::msg::BranchMsg::NewBranch { start: at, name } = create_branch(&row(branch("topic"), 1, Event::Other), &s) else { panic!() };
    assert_eq!((at, name.as_str()), (start, ""));
}
