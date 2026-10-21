//! Headless tests for undoing branch and tag creation, and a checkout that created a branch:
//! the dialog that offers to delete it, and redo making it again.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, UndoOpts};
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::{AppState, Modal};
use crate::tag::TagMsg;
use glowfly_core::undo::{Action, HeadPos};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A clean repository with one commit, open.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn has_branch(st: &AppState, name: &str) -> bool {
    st.snap.as_ref().unwrap().locals.iter().any(|b| b.name == name)
}

fn head_branch(st: &AppState) -> Option<String> {
    st.snap.as_ref().unwrap().head_branch.clone()
}

fn has_tag(st: &AppState, name: &str) -> bool {
    st.snap.as_ref().unwrap().tags.iter().any(|(n, _)| n == name)
}

fn undo_with(delete: bool, action: Action) -> Msg {
    Msg::Job(JobMsg::UndoWith { redo: false, action: Box::new(action), opts: UndoOpts { delete_created: delete, stash_first: false } })
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn new_branch_is_undoable() {
    let (mut st, dir) = opened("undo-refs-branch");
    drive(&mut st, Msg::git("Create branch", &["branch", "x"]));
    assert!(has_branch(&st, "x"));
    assert!(matches!(st.undo.undo.last(), Some(Action::CreateBranch { name, .. }) if name == "x"));
    assert!(st.undo.undo_entry(st.settings.lang(), true).0.starts_with("Undo creation of branch x"));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(!has_branch(&st, "x"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(has_branch(&st, "x"));
    cleanup(&[&dir]);
}

#[test]
fn new_tag_is_undoable_unless_pushed() {
    let (mut st, dir) = opened("undo-refs-tag");
    let at = st.snap.as_ref().unwrap().head_oid.unwrap().to_string();
    let create = |name: &str, push: Option<&str>| {
        Msg::Tag(TagMsg::CreateTag { at: at.clone(), name: name.into(), message: String::new(), push: push.map(Into::into) })
    };
    drive(&mut st, create("v1", None));
    assert!(has_tag(&st, "v1"));
    assert!(matches!(st.undo.undo.last(), Some(Action::CreateTag { name, .. }) if name == "v1"));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(!has_tag(&st, "v1"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(has_tag(&st, "v1"));

    let bare = dir.with_file_name(format!("{}-origin.git", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&dir, &["remote", "add", "origin", &bare.display().to_string()]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // Two commands, so nothing is recorded, and the tag job keeps the earlier entries.
    let fx = update(&mut st, create("v2", Some("origin")), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { cmds, record: None, .. }] if cmds.len() == 2), "{fx:?}");
    super::tests::finish(&mut st, fx);
    assert!(has_tag(&st, "v2"));
    assert!(matches!(st.undo.undo[..], [Action::CreateTag { .. }]), "{:?}", st.undo.undo);
    cleanup(&[&dir, &bare]);
}

#[test]
fn undo_of_a_new_branch_checkout_asks_to_delete_it() {
    let (mut st, dir) = opened("undo-refs-checkout-new");
    drive(&mut st, Msg::git("Create branch", &["checkout", "-b", "x"]));
    assert_eq!(head_branch(&st).as_deref(), Some("x"));
    let action = st.undo.undo.last().cloned().expect("recorded");
    assert!(matches!(&action, Action::Checkout { created: Some(c), .. } if c.name == "x"), "{action:?}");

    let fx = update(&mut st, Msg::Job(JobMsg::Undo), Instant::now());
    assert!(fx.is_empty() && st.job.is_none(), "the dialog asks first");
    let Some(Modal::UndoCreated(form)) = &st.modal else { panic!("no dialog") };
    assert!(form.delete && form.branch == "x" && form.back_to == "main", "{form:?}");

    drive(&mut st, undo_with(true, action.clone()));
    assert!(st.modal.is_none());
    assert_eq!(head_branch(&st).as_deref(), Some("main"));
    assert!(!has_branch(&st, "x"));
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Undo checkout of new branch x and delete branch x succeeded"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(st.modal.is_none(), "redo asks nothing");
    assert_eq!(head_branch(&st).as_deref(), Some("x"));

    // Kept when the box is unticked.
    drive(&mut st, undo_with(false, action.clone()));
    assert_eq!(head_branch(&st).as_deref(), Some("main"));
    assert!(has_branch(&st, "x"));
    drive(&mut st, Msg::Job(JobMsg::Redo));

    // A commit on the branch since: no dialog, just the switch back.
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "on x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(st.modal.is_none());
    assert_eq!(head_branch(&st).as_deref(), Some("main"));
    assert!(has_branch(&st, "x"));
    cleanup(&[&dir]);
}

#[test]
fn undo_with_runs_only_the_top_entry() {
    let (mut st, dir) = opened("undo-refs-not-top");
    drive(&mut st, Msg::git("Create branch", &["branch", "x"]));
    let other = Action::Checkout { from: HeadPos::Branch("x".into()), to: HeadPos::Branch("main".into()), created: None };
    let fx = update(&mut st, undo_with(false, other), Instant::now());
    assert!(fx.is_empty() && st.job.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "The undo history changed, so nothing was run."));
    assert!(has_branch(&st, "x") && st.undo.undo.len() == 1);
    cleanup(&[&dir]);
}
