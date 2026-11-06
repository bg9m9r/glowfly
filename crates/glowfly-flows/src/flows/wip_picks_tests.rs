//! Headless tests for WIP files picked with gaps, end to end through the runner with the test
//! build's stand-in trash: Delete discards the picks with one confirmation and one undo entry
//! (Undo brings every file back, Redo discards them again), Space stages and unstages only the
//! picked files, and a discard the trash stops partway keeps Undo for the files before it.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::repo::DiffTarget;
use glowfly_core::undo::{Action, BatchOp};
use glowfly_model::effect::{Effect, FilesEffect};
use glowfly_model::keymap::KeyAction;
use glowfly_model::msg::{JobMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::list_keys::ListKey;
use glowfly_model::state::wip_lists::{PickHow, picked_paths};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// a.txt and b.txt committed and edited, c.txt untracked (its text names the repository, so
/// the stand-in bin can tell it apart); a.txt open, then c.txt picked with Ctrl/Cmd (b.txt left
/// out).
fn picked_a_and_c(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "b"]);
    std::fs::write(dir.join("b.txt"), "b edited\n").unwrap();
    std::fs::write(dir.join("c.txt"), c_text(&dir)).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    drive(&mut st, pick(false, "c.txt"));
    assert_eq!(picked_paths(&st, false), ["a.txt", "c.txt"]);
    (st, dir)
}

fn c_text(dir: &Path) -> String {
    format!("untracked {}\n", dir.display())
}

fn pick(staged: bool, path: &str) -> Msg {
    Msg::Repo(RepoMsg::PickWipFile { staged, path: path.into(), how: PickHow::Toggle })
}

fn wip_key(staged: bool, key: ListKey) -> Msg {
    Msg::Shortcut(KeyAction::WipList { staged, key })
}

/// `f`'s text with line ends as LF: git's checkout may write CRLF (`core.autocrlf`).
fn read(dir: &Path, f: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(f)).ok().map(|t| t.replace("\r\n", "\n"))
}

/// How many files in the stand-in bin hold `contents`.
fn in_bin(contents: &str) -> usize {
    let Ok(dir) = std::fs::read_dir(glowfly_runner::testing::test_bin()) else { return 0 };
    dir.flatten().filter(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t == contents)).count()
}

fn listed(st: &AppState, staged: bool) -> Vec<&str> {
    let snap = st.snap.as_ref().unwrap();
    let list = if staged { &snap.staged } else { &snap.unstaged };
    list.iter().map(|f| f.path.as_str()).collect()
}

#[test]
fn discard_picks_with_gaps_then_undo_and_redo() {
    let (mut st, dir) = picked_a_and_c("picks-discard");
    st.settings.delete_to_trash = true;
    let bin = in_bin(&c_text(&dir));
    drive(&mut st, wip_key(false, ListKey::Delete));
    assert!(st.modal.is_some(), "one confirmation");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\n"), "back to its index version");
    assert_eq!(read(&dir, "c.txt"), None);
    assert_eq!(in_bin(&c_text(&dir)), bin + 1, "in the stand-in trash");
    assert_eq!(read(&dir, "b.txt").as_deref(), Some("b edited\n"), "not picked, untouched");
    assert_eq!(st.session.undo.undo.len(), 1, "one undo entry: {:?}", st.session.undo);
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Batch { op: BatchOp::Discard, parts }) if parts.len() == 2));
    assert!(st.cues.wip_picks.is_none());

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\ntwo\n"));
    assert_eq!(read(&dir, "c.txt"), Some(c_text(&dir)));
    assert_eq!(st.session.undo.redo.len(), 1, "{:?}", st.session.undo);

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\n"));
    assert_eq!(read(&dir, "c.txt"), None, "discarded again");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Batch { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_and_unstage_picks_with_gaps() {
    let (mut st, dir) = picked_a_and_c("picks-stage");
    drive(&mut st, wip_key(false, ListKey::Toggle));
    assert_eq!(listed(&st, true), ["a.txt", "c.txt"]);
    assert_eq!(listed(&st, false), ["b.txt"], "only the picked files moved");

    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("a.txt".into()))));
    drive(&mut st, pick(true, "c.txt"));
    drive(&mut st, wip_key(true, ListKey::Toggle));
    assert!(listed(&st, true).is_empty());
    assert_eq!(listed(&st, false), ["a.txt", "b.txt", "c.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_discard_that_stops_partway_keeps_undo_for_the_files_before() {
    let (mut st, dir) = picked_a_and_c("picks-stopped");
    st.settings.delete_to_trash = true;
    drive(&mut st, wip_key(false, ListKey::Delete));
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    let run = fx.into_iter().find_map(|e| match e {
        Effect::Files(FilesEffect::DiscardPaths { workdir, files, mode }) => Some((workdir, files, mode)),
        _ => None,
    });
    let (workdir, files, mode) = run.expect("the multi-file discard");
    // The trash refuses c.txt, after a.txt was discarded.
    drive(&mut st, glowfly_runner::testing::discard_paths_refused_for_test(&workdir, files, mode));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\n"));
    assert_eq!(read(&dir, "c.txt"), Some(c_text(&dir)), "nothing changed for c.txt");
    let last = st.log.last().map(|l| l.text.as_str()).unwrap_or_default();
    assert!(last.starts_with("Discarded 1 file, then stopped at c.txt: "), "{last}");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Discard { path, .. }) if path == "a.txt"), "{:?}", st.session.undo);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\ntwo\n"));
    let _ = std::fs::remove_dir_all(&dir);
}
