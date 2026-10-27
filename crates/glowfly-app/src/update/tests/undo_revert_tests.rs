//! Headless tests for undoing and redoing reverts of a commit's changes in the working tree
//! (Revert Hunk, Revert lines and Revert changes to this file): recorded in the tab's undo
//! history, undone to the exact working copy from before, redone, and recorded in a parked tab
//! when the revert finishes there.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{DiffMsg, Inactive, JobMsg, Msg, RepoMsg, RevertWhat, StageMsg, TabsMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::lines::LineClick::Only;
use glowfly_core::repo::{DiffTarget, Reverted};
use glowfly_core::undo::{Action, Discarded};
use std::path::{Path, PathBuf};
use std::time::Instant;

const OLD: &str = "a\n    x\n    y\nb\n";
/// `OLD` with `x` and `y` wrapped in a block, re-indented.
const NEW: &str = "a\nif {\n        x\n        y\n}\nb\n";

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// The scratch repository with `f.txt` committed as `old`, then as `new` (HEAD); the
/// repository is open with that commit's diff of `f.txt` on screen.
fn committed(name: &str, old: &str, new: &str, ignore_ws: bool) -> (AppState, PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["checkout", "--", "a.txt"]);
    std::fs::write(dir.join("f.txt"), old).unwrap();
    git(&dir, &["add", "f.txt"]);
    git(&dir, &["commit", "-q", "-m", "old"]);
    std::fs::write(dir.join("f.txt"), new).unwrap();
    git(&dir, &["commit", "-q", "-am", "new"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let target = DiffTarget::Commit(head, "f.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    if ignore_ws {
        drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    }
    (st, dir, target)
}

fn read(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("f.txt")).unwrap()
}

fn revert_hunk(st: &AppState) -> Msg {
    Msg::Stage(StageMsg::RevertHunk { index: 0, generation: st.hunk_ops_gen().expect("buttons on") })
}

fn said(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text)) || st.toasts.iter().any(|t| t.text.contains(text))
}

fn top_is(st: &AppState, want: Discarded) -> bool {
    matches!(st.undo.undo.last(), Some(Action::Discard { what, path, .. }) if *what == want && path == "f.txt")
}

#[test]
fn revert_hunk_with_whitespace_hidden_undo_and_redo() {
    let (mut st, dir, _) = committed("undo-revert-hunk", OLD, NEW, true);
    let msg = revert_hunk(&st);
    let fx = run(&mut st, msg);
    assert!(matches!(&fx[..], [Effect::Revert { req, .. }] if req.ignore_whitespace), "{fx:?}");
    finish(&mut st, fx);
    let reverted = "a\n        x\n        y\nb\n";
    assert_eq!(read(&dir), reverted, "only the shown change is reverted");
    assert!(top_is(&st, Discarded::RevertedHunk), "{:?}", st.undo.undo);
    let lang = st.settings.lang();
    assert_eq!(st.undo.undo_entry(lang, true).0, "Undo revert of a hunk in f.txt");
    assert_eq!(st.undo.undo_entry(lang, true).0, "Undo revert of a hunk in f.txt");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir), NEW, "the working copy from before, exactly");
    assert!(said(&st, "Restored f.txt"));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(read(&dir), reverted);
    assert!(said(&st, "Reverted the hunk in f.txt again"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revert_lines_and_file_are_recorded() {
    let (mut st, dir, target) = committed("undo-revert-lines", OLD, NEW, true);
    let close = st.session.diff.view.as_ref().unwrap().1.hunks[0].lines.iter().position(|l| l.origin == '+' && l.text == "}").unwrap();
    run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: close, how: Only }));
    drive(&mut st, Msg::Stage(StageMsg::RevertLines));
    assert_eq!(read(&dir), "a\nif {\n        x\n        y\nb\n");
    assert!(top_is(&st, Discarded::RevertedLines(1)), "{:?}", st.undo.undo);
    assert_eq!(st.undo.undo_entry(st.settings.lang(), true).0, "Undo revert of 1 line in f.txt");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir), NEW);

    drive(&mut st, Msg::Stage(StageMsg::RevertFile(target)));
    assert_eq!(read(&dir), OLD, "the whole change, whitespace too");
    assert!(top_is(&st, Discarded::RevertedFile), "{:?}", st.undo.undo);
    assert_eq!(st.undo.undo_entry(st.settings.lang(), true).0, "Undo revert of the changes to f.txt");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir), NEW);
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(read(&dir), OLD);
    assert!(said(&st, "Reverted the changes to f.txt again"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_revert_with_conflicts_undoes_to_the_edited_copy() {
    let (mut st, dir, _) = committed("undo-revert-conflict", "1\n2\n3\n", "1\nTWO\n3\n", false);
    std::fs::write(dir.join("f.txt"), "1\nTwo!\n3\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let msg = revert_hunk(&st);
    drive(&mut st, msg);
    assert!(read(&dir).contains("<<<<<<<"), "{}", read(&dir));
    assert!(top_is(&st, Discarded::RevertedHunk), "{:?}", st.undo.undo);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir), "1\nTwo!\n3\n", "the markers are gone");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_after_the_revert_refuses_the_undo() {
    let (mut st, dir, _) = committed("undo-revert-stale", OLD, NEW, false);
    let msg = revert_hunk(&st);
    drive(&mut st, msg);
    assert_eq!(read(&dir), OLD);
    std::fs::write(dir.join("f.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir), "edited\n", "left alone");
    assert!(said(&st, "has changed since"), "{:?}", st.log.last().map(|l| &l.text));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_revert_for_a_parked_tab_goes_to_its_history() {
    let (mut st, a, _) = committed("undo-revert-tab-a", OLD, NEW, false);
    let b = scratch_repo("undo-revert-tab-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let effect = run(&mut st, Msg::Stage(StageMsg::RevertFile(DiffTarget::Commit(head, "f.txt".into()))))
        .into_iter()
        .find(|e| matches!(e, Effect::Revert { .. }))
        .expect("a revert");
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    let reply = runner::execute(effect).expect("a result");
    assert!(matches!(reply, Msg::Scoped { .. }), "scoped to tab A's repository");
    run(&mut st, reply);
    assert_eq!(read(&a), OLD);
    assert!(st.undo.undo.is_empty(), "tab B's history is untouched");
    let parked = st.tabs[0].parked.as_ref().expect("tab A is parked");
    assert!(matches!(parked.undo.undo.last(), Some(Action::Discard { what: Discarded::RevertedFile, .. })), "{:?}", parked.undo.undo);
    for d in [&a, &b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn parked_results_keep_their_undo() {
    let undo = Action::Discard { path: "f.txt".into(), what: Discarded::RevertedHunk, before: None, after: None };
    let reverted = |result: Result<Reverted, String>, undo: Option<Action>| {
        StageMsg::Reverted { path: "f.txt".into(), what: RevertWhat::Hunk, result, undo }.on_inactive()
    };
    for r in [Reverted::Applied, Reverted::Removed, Reverted::Merged] {
        assert!(matches!(reverted(Ok(r), Some(undo.clone())), Some(Inactive::Undo(a)) if a == undo));
        assert!(reverted(Ok(r), None).is_none());
    }
    let conflicts = crate::update::revert::done_text("f.txt", RevertWhat::Hunk, Reverted::Conflicts);
    assert!(
        matches!(reverted(Ok(Reverted::Conflicts), Some(undo.clone())), Some(Inactive::LogAndUndo { text, undo: a }) if text == conflicts && a == undo)
    );
    assert!(matches!(reverted(Ok(Reverted::Conflicts), None), Some(Inactive::Log(text)) if text == conflicts));
    assert!(matches!(reverted(Err("boom".into()), None), Some(Inactive::Log(text)) if text == "Could not revert the hunk in f.txt: boom"));
}
