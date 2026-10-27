//! Headless tests for undoing merges, pulls and cherry-picks: recorded from the jobs that run
//! them, undone and redone end to end, and refused for local changes without a stash offer.

use super::cherry_pick_tests::feature_repo;
use super::selection_tests::git as git_out;
use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{CommitMsg, JobMsg, Msg, RepoMsg, SelectionMsg, StageMsg};
use crate::state::AppState;
use glowfly_core::undo::{Action, MoveOp};
use std::path::{Path, PathBuf};

fn head_summary(st: &AppState) -> String {
    let snap = st.snap.as_ref().expect("a repository is open");
    snap.commits[snap.row_of[&snap.head_oid.expect("HEAD")]].summary.to_string()
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
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Moved { op: MoveOp::Merge, .. })), "{:?}", st.session.undo.undo);
    assert!(
        st.session.undo.undo_entry(st.settings.lang(), true).0.starts_with("Undo merge of feature"),
        "{}",
        st.session.undo.undo_entry(st.settings.lang(), true).0
    );

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
    assert!(t.action.as_ref().is_none_or(|a| a.label == "Details"), "only hard resets and Discard all offer to stash");
    assert_eq!(st.session.undo.undo.len(), 1);
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
    st.session.draft.summary = "Second line".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(st.session.undo.undo.len(), 1);

    drive(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    assert!(st.toasts.last().is_some_and(|t| t.ok), "the pull ran");
    assert!(matches!(st.session.undo.undo[..], [Action::Commit { .. }]), "already up to date: {:?}", st.session.undo.undo);
    assert!(st.session.undo.blocked.is_none());
    cleanup(&[&dir, &bare]);
}

#[test]
fn cherry_pick_from_the_graph_is_undoable() {
    let (mut st, dir, picks) = feature_repo("undo-ops-pick", &[("f1", "base\n1\n")], None);
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![picks[0]])));
    assert_eq!(head_summary(&st), "f1");
    let Some(Action::Moved { op: MoveOp::CherryPick, .. }) = st.session.undo.undo.last() else { panic!("{:?}", st.session.undo.undo) };
    assert!(
        st.session.undo.undo_entry(st.settings.lang(), true).0.starts_with(&format!("Undo cherry-pick of {}", &picks[0].to_string()[..7]))
    );
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "base");
    cleanup(&[&dir]);
}

#[test]
fn a_multi_commit_cherry_pick_is_one_undo_step() {
    let (mut st, dir, f) = feature_repo("undo-ops-pick-3", &[("f1", "base\n1\n"), ("f2", "base\n1\n2\n"), ("f3", "base\n1\n2\n3\n")], None);
    drive(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![f[0], f[1], f[2]])));
    assert_eq!(head_summary(&st), "f3");
    let tip = st.session.undo.undo_entry(st.settings.lang(), true).0;
    assert!(tip.starts_with("Undo cherry-pick of 3 commits"), "{tip}");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "base", "one step takes all three back");
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(git_out(&dir, &["log", "--format=%s", "-4", "main"]).trim().lines().collect::<Vec<_>>(), ["f3", "f2", "f1", "base"]);
    cleanup(&[&dir]);
}

#[test]
fn a_multi_commit_revert_is_one_undo_step() {
    let (mut st, dir, f) = feature_repo("undo-ops-revert-2", &[("f1", "base\n1\n"), ("f2", "base\n1\n2\n")], None);
    git(&dir, &["checkout", "-q", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Selection(SelectionMsg::Revert(vec![f[0], f[1]])));
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n"), "base\n");
    let tip = st.session.undo.undo_entry(st.settings.lang(), true).0;
    assert!(tip.starts_with("Undo revert of 2 commits"), "{tip}");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "f2");
    cleanup(&[&dir]);
}

#[test]
fn a_mainline_cherry_pick_names_the_merge() {
    let (mut st, dir, _) = feature_repo("undo-ops-pick-merge", &[("f1", "base\n1\n")], None);
    let base = git_out(&dir, &["rev-parse", "main"]).trim().to_owned();
    git(&dir, &["merge", "-q", "--no-ff", "feature", "-m", "merge feature"]);
    let merge = git_out(&dir, &["rev-parse", "HEAD"]).trim().to_owned();
    git(&dir, &["checkout", "-q", "-b", "other", &base]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let oid = git2::Oid::from_str(&merge).unwrap();
    drive(
        &mut st,
        Msg::Selection(SelectionMsg::PickWithMainline { op: crate::state::pick::PickOp::CherryPick, oids: vec![oid], mainline: 1 }),
    );
    assert_eq!(head_summary(&st), "merge feature");
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n"), "base\n1\n");
    let tip = st.session.undo.undo_entry(st.settings.lang(), true).0;
    assert!(tip.starts_with(&format!("Undo cherry-pick of {}", &merge[..7])), "{tip}");
    cleanup(&[&dir]);
}
