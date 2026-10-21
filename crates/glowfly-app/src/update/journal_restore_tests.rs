//! Headless tests for the journal's Restore: the job each kind of state runs (checked-out
//! branch, other branch, deleted branch, HEAD), the stash on a dirty tree, every refusal, Undo
//! after a restore and Create branch here.

use super::journal_tests::{open_loaded, opened};
use super::tests::{drive, finish, git};
use super::update;
use crate::effect::Effect;
use crate::journal::JournalMsg;
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::journal::{Op, Row, Subject, notes};
use glowfly_core::undo::{ResetMode, UndoKind};
use std::path::Path;
use std::time::Instant;

fn rev(dir: &Path, r: &str) -> Oid {
    let out = glowfly_core::gitcli::read(dir, &glowfly_core::gitcli::args(&["rev-parse", r]));
    Oid::from_str(out.stdout.trim()).unwrap()
}

/// The newest row of `subject` at `oid`.
fn row(st: &AppState, subject: &Subject, oid: Oid) -> Row {
    let rows = st.journal.as_ref().unwrap().rows();
    rows.iter().find(|r| r.subject == *subject && r.oid == oid).unwrap_or_else(|| panic!("a row at {oid}: {rows:#?}")).clone()
}

fn restore(st: &mut AppState, r: &Row) {
    update(st, Msg::Journal(JournalMsg::Restore(r.key())), Instant::now());
}

/// The open confirmation's title and button.
fn asked(st: &AppState) -> (String, String) {
    match &st.modal {
        Some(Modal::Confirm { title, button, .. }) => (title.clone(), button.clone()),
        _ => panic!("a confirmation is open"),
    }
}

/// Accepts the confirmation; returns the effects it starts.
fn accept(st: &mut AppState) -> Vec<Effect> {
    update(st, Msg::ConfirmAccepted, Instant::now())
}

/// The parts of a started git job these tests check.
struct GitJob<'a> {
    cmds: &'a Vec<Vec<String>>,
    record: &'a Option<UndoKind>,
    journal: &'a Option<Op>,
    expect: &'a Option<(String, Oid)>,
}

fn git_job(fx: &[Effect]) -> GitJob<'_> {
    match fx.iter().find(|e| matches!(e, Effect::RunGit { .. })) {
        Some(Effect::RunGit { cmds, record, journal, expect, .. }) => GitJob { cmds, record, journal, expect },
        _ => panic!("a git job"),
    }
}

fn reload(st: &mut AppState, dir: &Path) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
    open_loaded(st, dir);
}

#[test]
fn the_checked_out_branch_is_reset_hard_and_undo_puts_it_back() {
    let (mut st, dir) = opened("journal-restore-head");
    let (first, second) = (rev(&dir, "topic"), rev(&dir, "main"));
    open_loaded(&mut st, &dir);
    let r = row(&st, &Subject::Branch("main".into()), first);
    restore(&mut st, &r);
    assert_eq!(asked(&st), ("Restore main?".into(), "Restore".into()));
    let fx = accept(&mut st);
    let GitJob { cmds, record, journal, expect } = git_job(&fx);
    assert_eq!(cmds, &glowfly_core::undo::hard_reset_commands(&first.to_string()));
    assert_eq!(record, &Some(UndoKind::Reset(ResetMode::Hard)));
    assert!(matches!(journal, Some(Op::Restore { branch: Some(b), .. }) if b == "main"));
    assert_eq!(expect, &Some(("refs/heads/main".into(), second)));
    finish(&mut st, fx);
    assert_eq!(rev(&dir, "main"), first, "main is back at the old state");
    assert!(notes::read(&dir).iter().any(|n| matches!(&n.op, Op::Restore { .. })), "the restore left its own note");
    assert_eq!(st.undo.undo.len(), 1, "the restore is undoable");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(rev(&dir, "main"), second, "Undo puts it back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dirty_tree_is_stashed_in_the_same_dialog() {
    let (mut st, dir) = opened("journal-restore-dirty");
    let first = rev(&dir, "topic");
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    reload(&mut st, &dir);
    let r = row(&st, &Subject::Branch("main".into()), first);
    restore(&mut st, &r);
    assert_eq!(asked(&st).1, "Stash and restore");
    let fx = accept(&mut st);
    let cmds = git_job(&fx).cmds;
    assert_eq!(&cmds[0][..2], &["stash".to_owned(), "push".to_owned()], "the auto-stash goes first: {cmds:?}");
    assert!(st.auto_stash.making.is_some(), "the stash is made for this run and put back after");
    assert!(st.modal.is_none(), "one dialog only");
    finish(&mut st, fx);
    assert_eq!(rev(&dir, "main"), first);
    let text = std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(text, "one\nmine\n", "the changes are back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_that_is_not_checked_out_moves_with_update_ref_even_on_a_dirty_tree() {
    let (mut st, dir) = opened("journal-restore-other");
    let (first, second) = (rev(&dir, "topic"), rev(&dir, "main"));
    git(&dir, &["branch", "-f", "topic", "main"]);
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    reload(&mut st, &dir);
    let r = row(&st, &Subject::Branch("topic".into()), first);
    restore(&mut st, &r);
    assert_eq!(asked(&st), ("Restore topic?".into(), "Restore".into()));
    let fx = accept(&mut st);
    let GitJob { cmds, record, .. } = git_job(&fx);
    assert_eq!(cmds, &vec![glowfly_core::journal::restore::move_branch("topic", first, second)]);
    assert!(matches!(record, Some(UndoKind::SetBranch(_))));
    assert!(st.auto_stash.making.is_none(), "no stash");
    finish(&mut st, fx);
    assert_eq!(rev(&dir, "topic"), first);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_branch_is_made_again() {
    let (mut st, dir) = opened("journal-restore-deleted");
    let first = rev(&dir, "topic");
    drive(&mut st, Msg::git("Delete branch", &["branch", "-D", "topic"]));
    open_loaded(&mut st, &dir);
    let r = row(&st, &Subject::Branch("topic".into()), first);
    assert!(matches!(&r.note, Some(n) if matches!(&n.op, Op::DeleteBranch { .. })), "the delete's note row");
    restore(&mut st, &r);
    assert_eq!(asked(&st), ("Recreate branch topic?".into(), "Create branch".into()));
    let fx = accept(&mut st);
    assert_eq!(git_job(&fx).cmds, &vec![glowfly_core::journal::restore::create_branch("topic", first)]);
    finish(&mut st, fx);
    assert_eq!(rev(&dir, "topic"), first);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_head_state_checks_out_its_branch_or_detaches() {
    let (mut st, dir) = opened("journal-restore-checkout");
    let first = rev(&dir, "topic");
    git(&dir, &["checkout", "-q", "topic"]);
    git(&dir, &["checkout", "-q", "main"]);
    reload(&mut st, &dir);
    let r =
        st.journal.as_ref().unwrap().rows().iter().find(|r| r.subject == Subject::Head && r.oid == first).cloned().expect("the checkout");
    restore(&mut st, &r);
    assert_eq!(asked(&st), ("Check out this state?".into(), "Check out".into()));
    let fx = accept(&mut st);
    assert_eq!(git_job(&fx).cmds, &vec![glowfly_core::journal::restore::checkout_branch("topic")]);
    // Not run: the next restore starts from the same place.
    st.job = None;
    // topic moves on: the state is now only its commit.
    git(&dir, &["branch", "-f", "topic", "main"]);
    reload(&mut st, &dir);
    let r = row(&st, &Subject::Head, first);
    restore(&mut st, &r);
    let fx = accept(&mut st);
    assert_eq!(git_job(&fx).cmds, &vec![glowfly_core::journal::restore::checkout_detached(first)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refusals_say_why_and_open_nothing() {
    let (mut st, dir) = opened("journal-restore-refuse");
    let (first, second) = (rev(&dir, "topic"), rev(&dir, "main"));
    open_loaded(&mut st, &dir);
    let refused = |st: &mut AppState, r: &Row, why: &str| {
        restore(st, r);
        assert!(st.modal.is_none(), "{why}");
        assert!(st.log.last().is_some_and(|l| l.text == why), "{why}: {:?}", st.log.last().map(|l| &l.text));
    };
    let current = row(&st, &Subject::Branch("main".into()), second);
    refused(&mut st, &current, "main is already at this commit.");
    let old = row(&st, &Subject::Branch("main".into()), first);
    st.job = Some(crate::state::RunningJob {
        label: "x".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    refused(&mut st, &old, "Another operation is still running.");
    st.job = None;
    let gone = Row { summary: None, ..old.clone() };
    let why = crate::state::journal_restore::plan(st.snap.as_ref().unwrap(), &gone, false, st.settings.lang()).unwrap_err();
    assert_eq!(why, "This commit is no longer in the repository.");

    let wt = glowfly_core::paths::temp_dir().join(format!("trellis-journal-wt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["branch", "-f", "topic", "main"]);
    git(&dir, &["worktree", "add", "-q", &wt.to_string_lossy(), "topic"]);
    reload(&mut st, &dir);
    let topic = row(&st, &Subject::Branch("topic".into()), first);
    restore(&mut st, &topic);
    assert!(st.modal.is_none());
    let text = &st.log.last().unwrap().text;
    assert!(text.starts_with("topic is checked out in ") && text.ends_with("; restore it there."), "{text}");
    git(&dir, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_branch_here_opens_the_new_branch_dialog() {
    let (mut st, dir) = opened("journal-restore-create");
    let first = rev(&dir, "topic");
    open_loaded(&mut st, &dir);
    let r = row(&st, &Subject::Branch("topic".into()), first);
    update(&mut st, Msg::Journal(JournalMsg::CreateBranch(r.key())), Instant::now());
    let Some(Modal::NewBranch { start, name, .. }) = &st.modal else { panic!("the New branch dialog") };
    assert_eq!((start.as_deref(), name.as_str()), (Some(first.to_string().as_str()), ""));
    let _ = std::fs::remove_dir_all(&dir);
}
