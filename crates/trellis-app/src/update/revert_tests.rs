//! Headless tests for Revert Hunk on commit and comparison diffs: which diffs it applies to,
//! refusals (stale, whitespace hidden), the whole-file view, and the result handling.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::state::{AppState, WHITESPACE_BLOCKS_HUNK_OPS, WHITESPACE_BLOCKS_REVERT};
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{self, DiffLine, DiffTarget, FileDiff, Hunk, LoadDiffOptions, LoadOptions};
use trellis_core::settings::DiffMode;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn one_hunk() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let hunks = vec![Hunk { header: "@@ -1 +1 @@".into(), lines: vec![line('-', "b"), line('+', "c")] }];
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

/// A loaded scratch repository with `target` open, showing `diff`.
fn with_diff(name: &str, target: DiffTarget, diff: FileDiff) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.diff = Some((target, diff.into()));
    (st, dir)
}

fn head(st: &AppState) -> Oid {
    st.snap.as_ref().unwrap().commits[0].oid
}

fn revert(st: &mut AppState, index: usize) -> Vec<Effect> {
    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    run(st, Msg::Stage(StageMsg::RevertHunk { index, generation }))
}

fn expect_effect(st: &mut AppState, target: &DiffTarget) {
    let fx = revert(st, 0);
    let want = one_hunk().hunks[0].clone();
    assert!(matches!(&fx[..], [Effect::RevertHunk { target: t, index: 0, expected, .. }] if t == target && *expected == want), "{fx:?}");
    assert!(st.hunk_ops_gen().is_some(), "diff_gen is not bumped");
}

#[test]
fn revert_sends_the_effect_for_the_shown_commit_diff() {
    let (mut st, dir) = with_diff("revert-commit", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let target = DiffTarget::Commit(head(&st), "a.txt".into());
    st.diff = Some((target.clone(), one_hunk().into()));
    expect_effect(&mut st, &target);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_works_on_a_comparison() {
    let (mut st, dir) = with_diff("revert-between", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let target = DiffTarget::Between(head(&st), head(&st), "a.txt".into());
    st.diff = Some((target.clone(), one_hunk().into()));
    expect_effect(&mut st, &target);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_generation_is_refused() {
    let (mut st, dir) = with_diff("revert-stale", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    st.diff = Some((DiffTarget::Commit(head(&st), "a.txt".into()), one_hunk().into()));
    let generation = st.hunk_ops_gen().unwrap() + 1;
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation })).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("diff changed")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whitespace_blocks_revert() {
    let (mut st, dir) = with_diff("revert-ws", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    st.diff = Some((DiffTarget::Commit(head(&st), "a.txt".into()), one_hunk().into()));
    st.settings.diff_ignore_whitespace = true;
    assert_eq!(st.hunk_ops_blocked(), Some(WHITESPACE_BLOCKS_REVERT));
    assert!(revert(&mut st, 0).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == WHITESPACE_BLOCKS_REVERT));

    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), one_hunk().into()));
    assert_eq!(st.hunk_ops_blocked(), Some(WHITESPACE_BLOCKS_HUNK_OPS));
    st.diff = None;
    assert_eq!(st.hunk_ops_blocked(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn working_tree_or_missing_hunk_is_ignored() {
    let (mut st, dir) = with_diff("revert-ignored", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    assert!(revert(&mut st, 0).is_empty(), "working-tree diffs have no revert");
    st.diff = Some((DiffTarget::Staged("a.txt".into()), one_hunk().into()));
    assert!(revert(&mut st, 0).is_empty());
    st.diff = Some((DiffTarget::Commit(head(&st), "a.txt".into()), one_hunk().into()));
    assert!(revert(&mut st, 1).is_empty(), "no such hunk");
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_mode_uses_the_normal_hunk() {
    let dir = scratch_repo("revert-whole");
    let numbered = |edits: &[(usize, &str)]| -> String {
        (1..=30).map(|i| edits.iter().find(|(n, _)| *n == i).map_or(format!("{i}\n"), |(_, t)| format!("{t}\n"))).collect()
    };
    std::fs::write(dir.join("b.txt"), numbered(&[])).unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(dir.join("b.txt"), numbered(&[(2, "two"), (25, "twenty-five")])).unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "two edits"]);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.settings.diff_mode = DiffMode::WholeFile;
    let target = DiffTarget::Commit(head(&st), "b.txt".into());
    let diff = repo::load_diff_with(&dir, &target, LoadDiffOptions { whole_file: true, ..Default::default() }).unwrap();
    assert_eq!(diff.hunks.len(), 2);
    let whole_hunk = diff.whole.as_ref().expect("whole file loaded").diff.hunks[0].clone();
    let want = diff.hunks[1].clone();
    assert_ne!(want, whole_hunk);
    st.diff = Some((target, diff.into()));
    let fx = revert(&mut st, 1);
    assert!(matches!(&fx[..], [Effect::RevertHunk { index: 1, expected, .. }] if *expected == want), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_end_to_end() {
    let dir = scratch_repo("revert-e2e");
    // Exact bytes: with autocrlf the revert would write CRLF, as a checkout does.
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["commit", "-q", "-am", "two"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let target = DiffTarget::Commit(head(&st), "a.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let generation = st.hunk_ops_gen().expect("diff on screen");
    drive(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation }));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
    assert!(st.toasts.iter().any(|t| t.ok && t.text.contains("a.txt")), "{:?}", st.log.last().map(|l| &l.text));
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "a.txt"));
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&target), "the commit diff stays open");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hunk_reverted_results() {
    let (mut st, dir) = with_diff("revert-results", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let fx = run(&mut st, Msg::Stage(StageMsg::HunkReverted { path: "a.txt".into(), result: Ok(()) }));
    assert!(matches!(&fx[..], [Effect::LoadStatus { .. }]), "{fx:?}");
    finish(&mut st, fx);
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text.contains("Reverted the hunk in a.txt")));
    let fx = run(&mut st, Msg::Stage(StageMsg::HunkReverted { path: "a.txt".into(), result: Err("x".into()) }));
    assert!(matches!(&fx[..], [Effect::LoadStatus { .. }]), "{fx:?}");
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Could not revert the hunk in a.txt: x"));
    assert!(st.view.show_log);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scoped_failure_for_inactive_tab_is_logged() {
    let (mut st, dir) = with_diff("revert-scoped", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let msg = Msg::Stage(StageMsg::HunkReverted { path: "a.txt".into(), result: Err("x".into()) });
    let fx = run(&mut st, Msg::Scoped { workdir: "C:/elsewhere".into(), msg: Box::new(msg) });
    assert!(fx.is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("x") && l.text.contains("a.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}
