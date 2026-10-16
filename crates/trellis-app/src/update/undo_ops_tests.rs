//! Headless tests for undoing merges, pulls and cherry-picks: recorded from the jobs that run
//! them, undone and redone end to end, and refused for local changes without a stash offer.

use super::cherry_pick_tests::feature_repo;
use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{CommitMsg, JobMsg, Msg, RepoMsg, SelectionMsg, StageMsg};
use crate::state::AppState;
use std::path::{Path, PathBuf};
use trellis_core::undo::{Action, MoveOp};

fn head_summary(st: &AppState) -> String {
    let snap = st.snap.as_ref().expect("a repository is open");
    snap.commits[snap.row_of[&snap.head_oid.expect("HEAD")]].summary.clone()
}

/// A clean repository on main ("first") with `feature` one commit ahead ("on feature"), open.
fn with_feature(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(dir.join("f.txt"), "f\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "on feature"]);
    git(&dir, &["checkout", "-q", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn merge_undo_and_redo_end_to_end() {
    let (mut st, dir) = with_feature("undo-ops-merge");
    drive(&mut st, Msg::git("Merge", &["merge", "feature"]));
    assert_eq!(head_summary(&st), "on feature");
    assert!(matches!(st.undo.undo.last(), Some(Action::Moved { op: MoveOp::Merge, .. })), "{:?}", st.undo.undo);
    assert!(st.undo.undo_tooltip(&st.keymap).starts_with("Undo merge of feature"), "{}", st.undo.undo_tooltip(&st.keymap));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "first");
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("main"));
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Undo merge of feature succeeded"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(head_summary(&st), "on feature");

    // Local changes to a tracked file: refused, the entry kept, and no stash offered.
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Job(JobMsg::Undo));
    let t = st.toasts.last().expect("a toast");
    assert!(!t.ok && t.text.contains("Commit or stash your changes first"), "{}", t.text);
    assert!(t.action.is_none(), "only hard resets and Discard all offer to stash");
    assert_eq!(st.undo.undo.len(), 1);
    cleanup(&[&dir]);
}

#[test]
fn pull_that_changes_nothing_keeps_the_history() {
    let dir = scratch_repo("undo-ops-pull");
    let bare = dir.with_file_name(format!("{}-origin.git", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&dir, &["remote", "add", "origin", &bare.display().to_string()]);
    git(&dir, &["push", "-q", "-u", "origin", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.draft.summary = "Second line".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(st.undo.undo.len(), 1);

    drive(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    assert!(st.toasts.last().is_some_and(|t| t.ok), "the pull ran");
    assert!(matches!(st.undo.undo[..], [Action::Commit { .. }]), "already up to date: {:?}", st.undo.undo);
    assert!(st.undo.blocked.is_none());
    cleanup(&[&dir, &bare]);
}

#[test]
fn cherry_pick_from_the_graph_is_undoable() {
    let (mut st, dir, picks) = feature_repo("undo-ops-pick", &[("f1", "base\n1\n")], None);
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![picks[0]])));
    assert_eq!(head_summary(&st), "f1");
    let Some(Action::Moved { op: MoveOp::CherryPick, .. }) = st.undo.undo.last() else { panic!("{:?}", st.undo.undo) };
    assert!(st.undo.undo_tooltip(&st.keymap).starts_with(&format!("Undo cherry-pick of {}", &picks[0].to_string()[..7])));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "base");
    cleanup(&[&dir]);
}
