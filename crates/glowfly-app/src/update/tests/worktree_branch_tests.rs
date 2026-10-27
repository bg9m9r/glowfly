//! Headless tests for local branches checked out in another worktree: Checkout and Delete
//! refused with the words the menus show (also once the worktree's folder is missing), a free
//! branch checked out as an undoable job, the current worktree's own branch never counted, and
//! the palette offering no checkout of such a branch.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::palette::entries;
use crate::state::worktree_branches::{checkout_blocked, delete_blocked, elsewhere, local_tip_line};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A repository with branch `free` and a linked worktree on branch `side` in
/// `<repo>-worktrees/side`, opened in `st`. Returns (state, repository, worktree folder).
fn with_side(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let root = dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    let side = root.join("side");
    git(&dir, &["branch", "free"]);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &side.display().to_string()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, side)
}

fn cleanup(dir: &Path, side: &Path) {
    let _ = std::fs::remove_dir_all(side.parent().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("a log entry")
}

fn branch(m: BranchMsg) -> Msg {
    Msg::Branch(m)
}

#[test]
fn checkout_and_delete_refused_for_a_branch_in_another_worktree() {
    let (mut st, dir, side) = with_side("wtb-refused");
    let snap = st.snap.as_ref().unwrap();
    let why = checkout_blocked(snap, "side").expect("side is checked out elsewhere");
    assert!(why.starts_with("side is checked out in the worktree at ") && why.ends_with(". Open that worktree instead."), "{why}");
    let delete = delete_blocked(snap, "side").unwrap();
    assert!(delete.ends_with("Remove that worktree or check out another branch there first."), "{delete}");
    assert!(local_tip_line(elsewhere(snap, "side").unwrap()).starts_with("Checked out in the worktree at "));

    let fx = update(&mut st, branch(BranchMsg::Checkout("side".into())), Instant::now());
    assert!(fx.iter().all(|e| !matches!(e, Effect::RunGit { .. })), "{fx:?}");
    assert!(st.job.is_none());
    assert_eq!(last_log(&st), (false, why));

    let fx = update(&mut st, branch(BranchMsg::DeleteBranchAndRemote("side".into())), Instant::now());
    assert!(fx.is_empty() && st.job.is_none());
    assert_eq!(last_log(&st), (false, delete));

    let fx = update(&mut st, branch(BranchMsg::Checkout("main".into())), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(last_log(&st).1, "main is already checked out.");
    cleanup(&dir, &side);
}

#[test]
fn a_free_branch_is_checked_out_as_an_undoable_job() {
    let (mut st, dir, side) = with_side("wtb-free");
    super::tests::clean_tree(&dir);
    super::tests::drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Refresh));
    let fx = update(&mut st, branch(BranchMsg::Checkout("free".into())), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, record, .. }] => {
            assert_eq!(cmds, &[crate::state::branches::checkout_local("free")]);
            assert!(record.is_some(), "recorded for undo");
        }
        other => panic!("expected one RunGit, got {other:?}"),
    }
    super::tests::finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("free"));
    assert_eq!(st.session.undo.undo.len(), 1);
    cleanup(&dir, &side);
}

#[test]
fn only_other_worktrees_count_and_a_missing_one_says_prune() {
    let (mut st, dir, side) = with_side("wtb-missing");
    let snap = st.snap.as_ref().unwrap();
    assert!(elsewhere(snap, "main").is_none(), "this worktree's own branch");
    assert!(elsewhere(snap, "free").is_none());

    let labels: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    assert!(labels.iter().any(|l| l == "Checkout free"), "{labels:?}");
    assert!(!labels.iter().any(|l| l == "Checkout side"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Open worktree side (side)"), "{labels:?}");

    std::fs::remove_dir_all(&side).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert!(elsewhere(snap, "side").is_some_and(|w| w.missing), "a missing worktree still counts");
    let why = checkout_blocked(snap, "side").unwrap();
    assert!(why.ends_with(", whose folder is missing. Prune it first."), "{why}");
    assert_eq!(delete_blocked(snap, "side").unwrap(), why);
    assert!(local_tip_line(elsewhere(snap, "side").unwrap()).ends_with(" (folder missing; prune it first)"));
    update(&mut st, branch(BranchMsg::Checkout("side".into())), Instant::now());
    assert_eq!(last_log(&st), (false, why));
    let labels: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    assert!(!labels.iter().any(|l| l.starts_with("Open worktree side")), "{labels:?}");
    cleanup(&dir, &side);
}
