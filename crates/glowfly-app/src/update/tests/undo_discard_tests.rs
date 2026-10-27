//! Headless tests for undoing hunk and line discards from the Unstaged diff: the label names
//! what was discarded, undo and redo write the file back exactly, several discards undo in
//! order, and a file edited since is never overwritten.

use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{DiffMsg, JobMsg, Msg, RepoMsg, StageMsg};
use crate::state::AppState;
use crate::state::lines::LineClick::Only;
use glowfly_core::repo::{DiffTarget, HunkOp};
use glowfly_core::undo::{Action, Discarded};
use std::path::{Path, PathBuf};

/// Thirty numbered lines, with an "x" before the ones in `changed`.
fn numbered(changed: &[usize]) -> String {
    (1..=30).map(|i| if changed.contains(&i) { format!("x{i}\n") } else { format!("{i}\n") }).collect()
}

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap()
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

/// big.txt committed as `numbered(&[])` and edited at lines 2 and 28 (two separate hunks), opened
/// with its Unstaged diff showing.
fn two_hunks(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]); // compare file contents byte for byte
    std::fs::write(dir.join("big.txt"), numbered(&[])).unwrap();
    git(&dir, &["add", "big.txt"]);
    git(&dir, &["commit", "-q", "-m", "big"]);
    std::fs::write(dir.join("big.txt"), numbered(&[2, 28])).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("big.txt".into()))));
    assert_eq!(st.session.diff.view.as_ref().map(|(_, d)| d.hunks.len()), Some(2));
    (st, dir)
}

fn discard_first_hunk(st: &mut AppState) {
    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    drive(st, Msg::Stage(StageMsg::Hunk { op: HunkOp::Discard, path: "big.txt".into(), index: 0, generation }));
}

#[test]
fn hunk_discard_undo_and_redo() {
    let (mut st, dir) = two_hunks("undo-discard-hunk");
    discard_first_hunk(&mut st);
    assert_eq!(read(&dir, "big.txt"), numbered(&[28]));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Discard { what: Discarded::Hunk, .. })), "{:?}", st.session.undo.undo);
    let tip = st.session.undo.undo_entry(st.settings.lang(), true).0;
    assert_eq!(tip, "Undo discard of a hunk in big.txt");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "big.txt"), numbered(&[2, 28]), "the exact text comes back");
    assert_eq!(last_toast(&st), (true, "Undo discard of a hunk in big.txt succeeded".to_owned()));
    assert_eq!(st.session.undo.redo.len(), 1);
    assert!(st.log.iter().any(|l| l.text.contains("Restored big.txt")));

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(read(&dir, "big.txt"), numbered(&[28]), "the other hunk stays");
    assert!(st.log.iter().any(|l| l.text.contains("Discarded the hunk in big.txt again")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn line_discard_undo_and_redo() {
    let dir = scratch_repo("undo-discard-lines");
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    let (_, d) = st.session.diff.view.as_ref().unwrap();
    let two = d.hunks[0].lines.iter().position(|l| l.origin == '+' && l.text == "two").unwrap();
    drive(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: two, how: Only }));
    drive(&mut st, Msg::Stage(StageMsg::ApplyLines(HunkOp::Discard)));
    assert_eq!(read(&dir, "a.txt"), "one\nthree\n");
    let top = st.session.undo.undo.last().expect("recorded");
    assert!(matches!(top, Action::Discard { what: Discarded::Lines(1), .. }), "{top:?}");
    assert_eq!(top.describe(), "discard of 1 line in a.txt");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt"), "one\ntwo\nthree\n");
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(read(&dir, "a.txt"), "one\nthree\n");
    assert!(st.log.iter().any(|l| l.text.contains("Discarded the lines in a.txt again")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_hunk_discards_undo_in_order() {
    let (mut st, dir) = two_hunks("undo-discard-two");
    discard_first_hunk(&mut st);
    discard_first_hunk(&mut st);
    assert_eq!(read(&dir, "big.txt"), numbered(&[]));
    let discards = st.session.undo.undo.iter().filter(|a| matches!(a, Action::Discard { .. })).count();
    assert_eq!(discards, 2, "{:?}", st.session.undo.undo);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "big.txt"), numbered(&[28]));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "big.txt"), numbered(&[2, 28]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edited_since_is_refused_and_clears() {
    let (mut st, dir) = two_hunks("undo-discard-edited");
    discard_first_hunk(&mut st);
    std::fs::write(dir.join("big.txt"), "newer work\n").unwrap();
    drive(&mut st, Msg::Job(JobMsg::Undo));
    let (ok, text) = last_toast(&st);
    assert!(!ok && text.contains("has changed since") && text.contains("git cat-file -p"), "{text}");
    assert!(st.session.undo.undo.is_empty(), "{:?}", st.session.undo.undo);
    assert_eq!(read(&dir, "big.txt"), "newer work\n");
    let _ = std::fs::remove_dir_all(&dir);
}
