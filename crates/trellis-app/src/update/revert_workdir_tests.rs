//! Headless tests for Revert Hunk and Revert lines on a commit compared with the working tree:
//! the effect sent, the hunk buttons waiting for the reload, Revert changes to this file and the
//! reverse direction refused, the R key, the new toast, and end to end (undo included).

use super::revert::{DIFF_CHANGED, done_text};
use super::revert_tests::{head, one_hunk, run, with_diff};
use super::tests::{drive, git, scratch_repo, state};
use crate::effect::Effect;
use crate::keymap::KeyAction;
use crate::msg::{JobMsg, Msg, RepoMsg, RevertWhat, SelectionMsg, StageMsg};
use crate::state::{AppState, Selection};
use trellis_core::repo::{DiffTarget, RevertPart, RevertRequest, Reverted};

fn parts(fx: &[Effect]) -> Vec<&RevertPart> {
    fx.iter().filter_map(|e| if let Effect::Revert { req: RevertRequest { part, .. }, .. } = e { Some(part) } else { None }).collect()
}

fn select_line(st: &mut AppState, target: &DiffTarget) {
    st.view.diff_lines.target = Some(target.clone());
    st.view.diff_lines.hunk = 0;
    st.view.diff_lines.lines.insert(1);
}

#[test]
fn reverting_on_a_commit_compared_with_the_working_tree() {
    let (mut st, dir) = with_diff("revert-workdir-effect", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let target = DiffTarget::ToWorkdir(head(&st), "a.txt".into());
    st.diff = Some((target.clone(), one_hunk().into()));
    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    let fx = run(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation }));
    assert!(matches!(&parts(&fx)[..], [RevertPart::Hunk { index: 0, .. }]), "{fx:?}");
    assert_eq!(st.hunk_ops_gen(), None, "the hunks wait for the reload");
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation })).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(DIFF_CHANGED));

    st.diff_shown_gen = st.diff_gen;
    select_line(&mut st, &target);
    let fx = run(&mut st, Msg::Stage(StageMsg::RevertLines));
    assert!(matches!(&parts(&fx)[..], [RevertPart::Lines { hunk: 0, .. }]), "{fx:?}");
    assert_eq!(st.hunk_ops_gen(), None);
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertFile(target))).is_empty(), "Restore covers the whole file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_r_key_reverts_the_current_hunk() {
    let (mut st, dir) = with_diff("revert-workdir-key", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    st.diff = Some((DiffTarget::ToWorkdir(head(&st), "a.txt".into()), one_hunk().into()));
    let fx = run(&mut st, Msg::Shortcut(KeyAction::RevertHunk));
    assert!(matches!(&parts(&fx)[..], [RevertPart::Hunk { index: 0, .. }]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_reverts_from_the_working_tree_to_a_commit() {
    let (mut st, dir) = with_diff("revert-workdir-reverse", DiffTarget::Unstaged("a.txt".into()), one_hunk());
    let target = DiffTarget::FromWorkdir(head(&st), "a.txt".into());
    st.diff = Some((target.clone(), one_hunk().into()));
    let generation = st.hunk_ops_gen().expect("a current diff");
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation })).is_empty(), "no Revert Hunk");
    assert!(run(&mut st, Msg::Shortcut(KeyAction::RevertHunk)).is_empty(), "no R");
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertFile(target.clone()))).is_empty(), "no Revert changes to this file");
    select_line(&mut st, &target);
    assert!(run(&mut st, Msg::Stage(StageMsg::RevertLines)).is_empty(), "no Revert lines");
    assert!(st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_the_compared_commit_lacks_is_removed() {
    let text = done_text("new.txt", RevertWhat::Hunk, Reverted::RemovedNew);
    assert_eq!(text, "Removed new.txt, which the compared commit doesn't have. Undo brings it back.");
}

fn numbered(edits: &[(usize, &str)]) -> String {
    (1..=20).map(|i| edits.iter().find(|(n, _)| *n == i).map_or(format!("{i}\n"), |(_, t)| format!("{t}\n"))).collect()
}

#[test]
fn a_hunk_reverted_end_to_end() {
    let dir = scratch_repo("revert-workdir-e2e");
    // The comparison is of exact bytes: no line-ending conversion from the system's config.
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("f.txt"), numbered(&[])).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(dir.join("f.txt"), numbered(&[(2, "two"), (18, "eighteen")])).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let c = head(&st);
    drive(&mut st, Msg::Selection(SelectionMsg::CompareWithWorkdir(c)));
    let target = DiffTarget::ToWorkdir(c, "f.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    assert_eq!(st.diff.as_ref().map(|(_, d)| d.hunks.len()), Some(2));
    let read = match &st.selection {
        Selection::VsWorkdir(w) => w.generation,
        other => panic!("not compared: {other:?}"),
    };

    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    drive(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation }));
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), numbered(&[(18, "eighteen")]));
    let (shown, diff) = st.diff.clone().expect("still open");
    assert_eq!(shown, target);
    assert_eq!(diff.hunks.len(), 1, "the diff reloaded");
    assert!(st.hunk_ops_gen().is_some(), "the buttons are back");
    match &st.selection {
        Selection::VsWorkdir(w) => assert!(w.generation > read && w.changes.as_ref().is_some_and(|c| c.is_ok()), "the list was read again"),
        other => panic!("not compared: {other:?}"),
    }
    assert!(st.undo.undo.last().is_some(), "an undo is recorded");
    assert!(st.log.iter().all(|l| l.ok), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_untracked_file_reverted_away_comes_back_with_undo() {
    let dir = scratch_repo("revert-workdir-untracked");
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(
        dir.join("f.txt"),
        "f
",
    )
    .unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(
        dir.join("new.txt"),
        "new
lines
",
    )
    .unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let c = head(&st);
    drive(&mut st, Msg::Selection(SelectionMsg::CompareWithWorkdir(c)));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::ToWorkdir(c, "new.txt".into()))));
    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    drive(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation }));
    assert!(!dir.join("new.txt").exists(), "the file the commit lacks is removed");
    let want = done_text("new.txt", RevertWhat::Hunk, Reverted::RemovedNew);
    assert!(st.toasts.iter().any(|t| t.ok && t.text == want), "{:?}", st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(
        std::fs::read_to_string(dir.join("new.txt")).unwrap(),
        "new
lines
",
        "undo brings it back"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
