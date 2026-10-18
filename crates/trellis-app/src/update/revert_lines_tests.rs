//! Headless tests for Revert lines (a line selection on a commit or comparison diff), Revert
//! changes to this file, and the File history rename mapping they share with Revert Hunk.

use super::revert_tests::{head, one_hunk, run, with_diff};
use super::tests::{drive, git, scratch_repo, state};
use crate::effect::Effect;
use crate::msg::{DiffMsg, Msg, RepoMsg, StageMsg};
use crate::state::history::FileHistoryView;
use crate::state::lines::LineClick;
use crate::state::{AppState, WHITESPACE_BLOCKS_REVERT, WHOLE_FILE_BLOCKS_LINE_OPS};
use git2::Oid;
use trellis_core::repo::{DiffTarget, HistoryEntry, RevertPart, RevertRequest};
use trellis_core::settings::DiffMode;

fn select(st: &mut AppState, line: usize) {
    assert!(run(st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how: LineClick::Only })).is_empty());
}

fn requests(fx: &[Effect]) -> Vec<&RevertRequest> {
    fx.iter().filter_map(|e| if let Effect::Revert { req, .. } = e { Some(req) } else { None }).collect()
}

#[test]
fn revert_lines_sends_effect() {
    let (mut st, dir) = with_diff("revert-lines-fx", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let target = DiffTarget::Commit(head(&st), "a.txt".into());
    st.diff = Some((target.clone(), one_hunk().into()));
    select(&mut st, 1);
    assert!(!st.view.diff_lines.is_empty());
    let generation = st.hunk_ops_gen();
    let fx = run(&mut st, Msg::Stage(StageMsg::RevertLines));
    let expected = one_hunk().hunks[0].clone();
    let want = RevertRequest { target, part: RevertPart::Lines { hunk: 0, expected, lines: vec![1] }, dest: None };
    assert_eq!(requests(&fx), [&want], "{fx:?}");
    assert_eq!(fx.len(), 1);
    assert!(st.view.diff_lines.is_empty(), "the selection is cleared");
    assert_eq!(st.hunk_ops_gen(), generation, "diff_gen is not bumped");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_lines_ignored() {
    let (mut st, dir) = with_diff("revert-lines-ignored", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let commit = DiffTarget::Commit(head(&st), "a.txt".into());
    st.diff = Some((commit.clone(), one_hunk().into()));
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertLines)).is_empty(), "nothing selected");

    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), one_hunk().into()));
    select(&mut st, 1);
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertLines)).is_empty(), "a working-tree diff");

    st.diff = Some((commit.clone(), one_hunk().into()));
    select(&mut st, 1);
    st.diff = Some((DiffTarget::Commit(head(&st), "b.txt".into()), one_hunk().into()));
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertLines)).is_empty(), "a selection of another diff");
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_lines_refused() {
    let (mut st, dir) = with_diff("revert-lines-refused", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    st.diff = Some((DiffTarget::Commit(head(&st), "a.txt".into()), one_hunk().into()));
    select(&mut st, 1);
    st.settings.diff_ignore_whitespace = true;
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertLines)).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == WHITESPACE_BLOCKS_REVERT));
    st.settings.diff_ignore_whitespace = false;

    st.settings.diff_mode = DiffMode::WholeFile;
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertLines)).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == WHOLE_FILE_BLOCKS_LINE_OPS));
    st.settings.diff_mode = DiffMode::Hunks;

    st.diff_gen += 1;
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertLines)).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("diff changed")));
    assert!(!st.view.diff_lines.is_empty(), "a refusal keeps the selection");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_file_sends_effect() {
    let (mut st, dir) = with_diff("revert-file-fx", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let (a, b) = (head(&st), Oid::zero());
    for target in [DiffTarget::Commit(a, "a.txt".into()), DiffTarget::Between(b, a, "a.txt".into())] {
        let fx = run(&mut st, Msg::Stage(StageMsg::RevertFile(target.clone())));
        let want = RevertRequest { target, part: RevertPart::File, dest: None };
        assert_eq!(requests(&fx), [&want], "{fx:?}");
    }
    // Whatever diff is open (or none), and with whitespace hidden: it doesn't use the diff.
    st.diff = None;
    st.settings.diff_ignore_whitespace = true;
    assert_eq!(requests(&run(&mut st, Msg::Stage(StageMsg::RevertFile(DiffTarget::Commit(a, "a.txt".into()))))).len(), 1);
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertFile(DiffTarget::Unstaged("a.txt".into())))).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

fn entry(oid: Oid, path: &str) -> HistoryEntry {
    HistoryEntry {
        oid,
        summary: "edit".into(),
        author: "Test".into(),
        email: "test@example.com".into(),
        time: 1_700_000_000,
        path: path.into(),
        old_path: None,
        status: 'M',
        files: Vec::new(),
    }
}

#[test]
fn history_dest_mapping() {
    let (mut st, dir) = with_diff("revert-history-dest", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let (old, same) = (head(&st), Oid::from_bytes(&[7; 20]).unwrap());
    let mut view = FileHistoryView::loading("new.txt".into(), None);
    view.entries = vec![entry(same, "new.txt"), entry(old, "old.txt")];
    st.history = Some(view);
    let renamed = DiffTarget::Commit(old, "old.txt".into());
    st.diff = Some((renamed.clone(), one_hunk().into()));
    let generation = st.hunk_ops_gen().unwrap();
    let fx = run(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation }));
    assert_eq!(requests(&fx).iter().map(|r| r.dest.as_deref()).collect::<Vec<_>>(), [Some("new.txt")]);
    select(&mut st, 1);
    let fx = run(&mut st, Msg::Stage(StageMsg::RevertLines));
    assert_eq!(requests(&fx).iter().map(|r| r.dest.as_deref()).collect::<Vec<_>>(), [Some("new.txt")]);
    let fx = run(&mut st, Msg::Stage(StageMsg::RevertFile(renamed)));
    assert_eq!(requests(&fx).iter().map(|r| r.dest.as_deref()).collect::<Vec<_>>(), [Some("new.txt")]);
    // The entry under the history's own name, and a commit that isn't listed, keep their path.
    for target in [DiffTarget::Commit(same, "new.txt".into()), DiffTarget::Commit(same, "old.txt".into())] {
        let fx = run(&mut st, Msg::Stage(StageMsg::RevertFile(target)));
        assert_eq!(requests(&fx).iter().map(|r| r.dest.as_deref()).collect::<Vec<_>>(), [None]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A scratch repository whose HEAD commit adds `two` to `a.txt`, opened in `st`.
fn opened(name: &str) -> (AppState, std::path::PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    // Exact bytes: with autocrlf the revert would write CRLF, as a checkout does.
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["commit", "-q", "-am", "two"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let target = DiffTarget::Commit(head(&st), "a.txt".into());
    (st, dir, target)
}

#[test]
fn revert_lines_end_to_end() {
    let (mut st, dir, target) = opened("revert-lines-e2e");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let line = st.diff.as_ref().unwrap().1.hunks[0].lines.iter().position(|l| l.origin == '+').unwrap();
    drive(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how: LineClick::Only }));
    drive(&mut st, Msg::Stage(StageMsg::RevertLines));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
    assert!(st.toasts.iter().any(|t| t.ok && t.text.starts_with("Reverted 1 line in a.txt")), "{:?}", st.log.last().map(|l| &l.text));
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_file_end_to_end() {
    let (mut st, dir, target) = opened("revert-file-e2e");
    drive(&mut st, Msg::Stage(StageMsg::RevertFile(target)));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
    assert!(st.toasts.iter().any(|t| t.ok && t.text.starts_with("Reverted the changes to a.txt")), "{:?}", st.log.last().map(|l| &l.text));
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}
