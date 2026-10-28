//! Headless tests for the branch Delete dialog: a branch the checked-out one contains opens it
//! ready to Delete (`git branch -d`), anything else counts the commits only it has first; with
//! some, it offers Delete anyway (`-D`), with none (they are on another branch) a plain Delete
//! that still uses `-D`. Stale counts are dropped, a moved branch is refused, a checked-out one
//! never opens it, and Undo brings the deleted branch back.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{BranchEffect, Effect};
use crate::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use crate::state::branch_delete::{DeleteForm, Lost};
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use std::path::{Path, PathBuf};
use std::time::Instant;

const T: Lang = Lang::En;

/// A clean repository on main with `topic` three commits ahead of it, open.
fn with_topic(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    for f in ["t1.txt", "t2.txt", "t3.txt"] {
        std::fs::write(dir.join(f), f).unwrap();
        git(&dir, &["add", f]);
        git(&dir, &["commit", "-q", "-m", f]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn tip(st: &AppState, name: &str) -> Option<Oid> {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).and_then(|b| b.oid)
}

fn form(st: &AppState) -> &DeleteForm {
    match &st.modal {
        Some(Modal::DeleteBranch(f)) => f,
        _ => panic!("the Delete dialog is open"),
    }
}

fn open_dialog(st: &mut AppState, name: &str) -> Vec<Effect> {
    update(st, Msg::Branch(BranchMsg::DeleteDialog(name.into())), Instant::now())
}

/// Presses the dialog's danger button and returns the commands of the job it starts.
fn press(st: &mut AppState) -> (Vec<Effect>, Vec<Vec<String>>) {
    let delete = form(st).delete_msg().expect("the button is enabled");
    let fx = update(st, delete, Instant::now());
    let cmds = fx
        .iter()
        .find_map(|e| match e {
            Effect::RunGit { cmds, .. } => Some(cmds.clone()),
            _ => None,
        })
        .unwrap_or_default();
    (fx, cmds)
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn merged_branch_deletes_without_a_count() {
    let (mut st, dir) = with_topic("bd-merged");
    git(&dir, &["merge", "-q", "--ff-only", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = open_dialog(&mut st, "topic");
    assert!(fx.is_empty(), "main contains topic: nothing to count");
    let f = form(&st);
    assert_eq!((&f.lost, f.button(T), f.lost_note(T)), (&Lost::Known(0), "Delete", None));
    let (fx, cmds) = press(&mut st);
    assert!(st.modal.is_none(), "the dialog closes");
    assert_eq!(cmds, vec![args(&["branch", "-d", "topic"])]);
    finish(&mut st, fx);
    assert_eq!(tip(&st, "topic"), None);
    cleanup(&dir);
}

#[test]
fn unmerged_branch_counts_then_deletes_anyway_and_undo_brings_it_back() {
    let (mut st, dir) = with_topic("bd-unmerged");
    let at = tip(&st, "topic").unwrap();
    let fx = open_dialog(&mut st, "topic");
    assert!(matches!(&fx[..], [Effect::Branch(BranchEffect::CountUnmerged { name, tip, .. })] if name == "topic" && *tip == at));
    assert_eq!(form(&st).lost, Lost::Counting);
    assert!(form(&st).delete_msg().is_none(), "nothing to press while counting");
    finish(&mut st, fx);
    let f = form(&st);
    assert_eq!(f.lost, Lost::Known(3));
    assert_eq!(f.button(T), "Delete anyway");
    let note = "3 commits aren't on any other branch, so deleting topic leaves them on none. Undo can bring the branch back.";
    assert_eq!(f.lost_note(T).as_deref(), Some(note));

    let (fx, cmds) = press(&mut st);
    assert_eq!(cmds, vec![args(&["branch", "-D", "topic"])]);
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Delete branch"));
    finish(&mut st, fx);
    assert_eq!(tip(&st, "topic"), None);
    assert_eq!(st.session.undo.undo.len(), 1);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(tip(&st, "topic"), Some(at), "Undo puts the branch back at its tip");
    cleanup(&dir);
}

#[test]
fn branch_on_another_branch_offers_plain_delete() {
    let (mut st, dir) = with_topic("bd-elsewhere");
    git(&dir, &["branch", "copy", "topic"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = open_dialog(&mut st, "topic");
    finish(&mut st, fx);
    assert_eq!((&form(&st).lost, form(&st).button(T)), (&Lost::Known(0), "Delete"));
    // git branch -d would refuse: copy isn't HEAD or topic's upstream.
    assert_eq!(press(&mut st).1, vec![args(&["branch", "-D", "topic"])]);
    cleanup(&dir);
}

#[test]
fn stale_counts_are_dropped() {
    let (mut st, dir) = with_topic("bd-stale");
    let at = tip(&st, "topic").unwrap();
    let _ = open_dialog(&mut st, "topic");
    let counted = |name: &str, tip: Oid| Msg::Branch(BranchMsg::UnmergedCounted { name: name.into(), tip, count: Ok(1) });
    drive(&mut st, counted("other", at));
    drive(&mut st, counted("topic", Oid::zero()));
    assert_eq!(form(&st).lost, Lost::Counting, "another branch's or tip's count");
    drive(&mut st, Msg::Branch(BranchMsg::UnmergedCounted { name: "topic".into(), tip: at, count: Err("broken".into()) }));
    let f = form(&st);
    assert_eq!(f.lost_note(T).as_deref(), Some("Couldn't check whether its commits are on another branch: broken"));
    assert_eq!(f.button(T), "Delete anyway", "a failed count may lose work");
    drive(&mut st, Msg::CloseModal);
    drive(&mut st, counted("topic", at));
    assert!(st.modal.is_none(), "a count for a closed dialog opens nothing");
    cleanup(&dir);
}

#[test]
fn checked_out_or_moved_branches_are_refused() {
    let (mut st, dir) = with_topic("bd-refused");
    assert!(open_dialog(&mut st, "main").is_empty());
    assert!(st.modal.is_none());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("main is checked out. Check out another branch before deleting it."));

    // The dialog was opened for an older tip: the job's revision check refuses the delete.
    let at = tip(&st, "topic").unwrap();
    git(&dir, &["branch", "-f", "topic", "main"]);
    let fx = update(&mut st, Msg::Branch(BranchMsg::Delete { name: "topic".into(), tip: at, force: true }), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { expect: Some((r, o)), .. }] if r == "refs/heads/topic" && *o == at), "{fx:?}");
    finish(&mut st, fx);
    assert!(tip(&st, "topic").is_some(), "the moved branch stays");
    cleanup(&dir);
}
