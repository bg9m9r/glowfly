//! Headless tests for line operations that take in a whole new file: discarding every line of
//! an untracked file deletes it through Discard's trash and undo (never once the file changed
//! since the diff was shown; an intent-to-add file is emptied instead), a partial discard stays
//! a line discard, and unstaging every line of a staged new file follows it to its untracked
//! view.

use super::lines_tests::{click, loaded, run, two_hunks};
use super::tests::{drive, git, scratch_repo, state};
use super::work_files_tests::entry;
use crate::effect::{DeleteMode, Effect, FileOp, LocalOp, SeenLines};
use crate::msg::{JobMsg, Msg, RepoMsg, StageMsg};
use crate::state::AppState;
use crate::state::lines::LineClick::{Only, Range};
use glowfly_core::repo::{self, DiffTarget, HunkOp, LoadOptions};
use std::path::PathBuf;

/// A scratch repository holding the untracked u.txt with `contents`, opened, with its unstaged
/// diff on screen. `prepare` runs in the repository before it is opened.
fn untracked_open(name: &str, contents: &str, prepare: &[&[&str]]) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("u.txt"), contents).unwrap();
    for args in prepare {
        git(&dir, args);
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(unstaged_u())));
    assert!(st.session.diff.view.as_ref().is_some_and(|(t, _)| *t == unstaged_u()), "the diff is open");
    (st, dir)
}

fn unstaged_u() -> DiffTarget {
    DiffTarget::Unstaged("u.txt".into())
}

/// Selects the first `n` lines of hunk 0 (every one, for `n` = its length).
fn select_first(st: &mut AppState, n: usize) {
    click(st, 0, 0, Only);
    if n > 1 {
        click(st, 0, n - 1, Range);
    }
}

/// Selects every added line of hunk 0.
fn select_all(st: &mut AppState) {
    let n = st.session.diff.view.as_ref().unwrap().1.hunks[0].lines.iter().filter(|l| l.origin == '+').count();
    select_first(st, n);
}

fn discard_lines() -> Msg {
    Msg::Stage(StageMsg::ApplyLines(HunkOp::Discard))
}

fn last_toast(st: &AppState) -> &str {
    st.toasts.last().map(|t| t.text.as_str()).unwrap_or_default()
}

#[test]
fn every_line_of_an_untracked_file_is_a_discard_of_the_file() {
    let (mut st, dir) = untracked_open("lines-whole-op", "a\nb\nc\n", &[]);
    select_all(&mut st);
    let fx = run(&mut st, discard_lines());
    let [Effect::WorkFile { op: FileOp::Discard { path, mode: DeleteMode::Trash, seen: Some(seen) }, .. }] = &fx[..] else {
        panic!("a Discard of the file: {fx:?}")
    };
    assert_eq!(path, "u.txt");
    assert_eq!(seen.lines, vec![0, 1, 2]);
    assert!(st.modal.is_none(), "no confirmation: Undo brings it back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discarding_every_line_trashes_the_file_and_undo_brings_it_back() {
    for to_trash in [true, false] {
        let name = if to_trash { "lines-whole-trash" } else { "lines-whole-perm" };
        let contents = format!("one {name}\ntwo\nno newline");
        let (mut st, dir) = untracked_open(name, &contents, &[]);
        st.settings.delete_to_trash = to_trash;
        select_all(&mut st);
        drive(&mut st, discard_lines());
        assert!(!dir.join("u.txt").exists(), "deleted, not emptied");
        assert!(st.session.diff.view.is_none(), "its diff is closed");
        assert!(st.snap.as_ref().unwrap().unstaged.iter().all(|f| f.path != "u.txt"));
        assert!(last_toast(&st).contains("Undo"), "{}", last_toast(&st));

        drive(&mut st, Msg::Job(JobMsg::Undo));
        assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), contents, "{:?}", st.log);
        assert!(st.snap.as_ref().unwrap().unstaged.contains(&entry("u.txt", '?')));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_file_changed_since_the_diff_was_shown_is_never_deleted() {
    let (mut st, dir) = untracked_open("lines-whole-stale", "a\nb\n", &[]);
    select_all(&mut st);
    std::fs::write(dir.join("u.txt"), "a\nb\nnew work\n").unwrap();
    drive(&mut st, discard_lines());
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "a\nb\nnew work\n");
    let log = st.log.last().map(|l| l.text.as_str()).unwrap_or_default();
    assert!(log.starts_with("Could not discard u.txt:") && log.contains("the diff changed"), "{log}");
    assert!(st.session.undo.undo.is_empty(), "nothing to undo");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_partial_discard_stays_a_line_discard() {
    let (mut st, dir) = untracked_open("lines-whole-partial", "a\nb\nc\n", &[]);
    select_first(&mut st, 2);
    let fx = run(&mut st, discard_lines());
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Lines { op: HunkOp::Discard, .. }, .. }]), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_intent_to_add_file_is_emptied_not_deleted() {
    // libgit2's status lists it as staged and modified, git's as untracked (`status_cli`): a
    // line discard either way, and a Discard sent for it discards the lines instead.
    let (mut st, dir) = untracked_open("lines-whole-ita", "a\nb\n", &[&["add", "-N", "u.txt"]]);
    select_all(&mut st);
    drive(&mut st, discard_lines());
    assert_eq!(std::fs::read(dir.join("u.txt")).unwrap(), b"", "{:?}", st.log);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "a\nb\n", "{:?}", st.log);

    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(unstaged_u())));
    let expected = st.session.diff.view.as_ref().unwrap().1.hunks[0].clone();
    let seen = Some(Box::new(SeenLines { expected, lines: vec![0, 1], ignore_whitespace: false }));
    let msg = crate::runner::files::work_file_for_test(&dir, FileOp::Discard { path: "u.txt".into(), mode: DeleteMode::Trash, seen });
    assert!(matches!(msg, Msg::Stage(StageMsg::LocalOpDone(Ok(Some(_))))), "a line discard, with its undo");
    assert_eq!(std::fs::read(dir.join("u.txt")).unwrap(), b"");
    drive(&mut st, msg);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "a\nb\n", "{:?}", st.log);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstaging_every_line_of_a_new_file_follows_it_to_its_untracked_view() {
    let dir = scratch_repo("lines-whole-unstage");
    std::fs::write(dir.join("n.txt"), "one\ntwo\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("n.txt".into())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("n.txt".into()))));
    select_first(&mut st, 2);
    drive(&mut st, Msg::Stage(StageMsg::ApplyLines(HunkOp::Unstage)));
    let (target, d) = st.session.diff.view.as_ref().expect("still open");
    assert_eq!(*target, DiffTarget::Unstaged("n.txt".into()));
    assert_eq!(d.hunks[0].lines.iter().filter(|l| l.origin == '+').count(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_emptied_staged_diff_closes_unless_its_file_is_untracked() {
    let staged = DiffTarget::Staged("f.txt".into());
    let dir = scratch_repo("lines-whole-emptied");
    let showing = |dir: &std::path::Path| {
        let mut st = state();
        st.snap = Some(repo::load(dir, &LoadOptions::all(100)).unwrap());
        st.session.diff.view = Some((staged.clone(), two_hunks().into()));
        loaded(&mut st, staged.clone(), true, Err("no changes in f.txt".into()));
        st
    };
    // Gone from both lists (unstaged and deleted, say): closed.
    let st = showing(&dir);
    assert!(st.session.diff.view.is_none() && st.view.diff_opening.is_none());

    // Untracked now: its untracked view opens.
    std::fs::write(dir.join("f.txt"), "x\n").unwrap();
    let st = showing(&dir);
    assert_eq!(st.view.diff_opening, Some(DiffTarget::Unstaged("f.txt".into())));
    let _ = std::fs::remove_dir_all(&dir);
}
