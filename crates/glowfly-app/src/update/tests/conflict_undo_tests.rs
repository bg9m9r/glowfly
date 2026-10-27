//! Headless tests for undoing and redoing a resolution saved in the conflict view (and a save
//! with markers) through Edit > Undo: the conflict and the exact working bytes come back, the
//! view opens on the file again with the picks saved with markers, redo resolves again, and an
//! entry that expired is dropped alone.

use super::conflict_tests::{conflicted_repo, two_conflicts};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{CommitMsg, ConflictMsg, JobMsg, Msg, RepoMsg, StageMsg, TabsMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::{Choice, Side};
use glowfly_core::undo::Action;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn send(st: &mut AppState, m: ConflictMsg) {
    update(st, Msg::Conflict(m), Instant::now());
}

fn bytes(dir: &Path, f: &str) -> Vec<u8> {
    std::fs::read(dir.join(f)).unwrap()
}

fn view(st: &AppState) -> &ConflictView {
    st.session.conflict.view.as_ref().expect("view open")
}

fn conflicted(st: &AppState, path: &str) -> bool {
    st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!' && f.path == path))
}

fn staged(st: &AppState, path: &str) -> bool {
    st.snap.as_ref().is_some_and(|s| s.staged.iter().any(|f| f.path == path))
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|l| l.text.contains(text))
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

fn undo_label(st: &AppState) -> String {
    st.session.undo.undo_entry(st.settings.lang(), true).0
}

fn redo_label(st: &AppState) -> String {
    st.session.undo.redo_entry(st.settings.lang(), true).0
}

fn open(dir: &Path, file: &str) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict(file.into())));
    st
}

/// A repository mid-merge where f.txt conflicts in two chunks (lines 2 and 10).
fn two_chunks(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    let base: String = (1..=12).map(|i| format!("line {i}\n")).collect();
    let side = |s: &str| base.replace("line 2\n", &format!("line 2 {s}\n")).replace("line 10\n", &format!("line 10 {s}\n"));
    std::fs::write(dir.join("f.txt"), &base).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("f.txt"), side("theirs")).unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("f.txt"), side("ours")).unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

/// a.txt edited on main ("ours") and other ("theirs"), b.txt committed through the app so the
/// history holds a commit, then `merge other` run from the app (it stops for the conflict), with
/// the conflict view open on a.txt.
fn merging_after_a_commit(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("a.txt"), "one\ntheirs\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "one\nours\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("b.txt".into())));
    st.session.draft.summary = "add b".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    drive(&mut st, Msg::git("Merge", &["merge", "other"]));
    assert!(conflicted(&st, "a.txt"));
    assert!(matches!(&st.session.undo.undo[..], [Action::Commit { .. }]), "{:?}", st.session.undo.undo);
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    (st, dir)
}

#[test]
fn undo_and_redo_a_saved_resolution() {
    let dir = conflicted_repo("conflict-undo-resolve");
    let mut st = open(&dir, "a.txt");
    let markers = bytes(&dir, "a.txt");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Theirs });
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    assert!(st.session.conflict.view.is_none(), "resolved and closed");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Resolve { staged: Some(_), .. })), "{:?}", st.session.undo.undo);
    assert_eq!(undo_label(&st), "Undo resolution of a.txt");
    let tip = st.session.undo.undo_entry(st.settings.lang(), true).0;
    assert_eq!(tip, "Undo resolution of a.txt");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(bytes(&dir, "a.txt"), markers);
    assert!(conflicted(&st, "a.txt"));
    let c = view(&st);
    assert_eq!(c.path, "a.txt");
    assert!(c.file.is_some(), "loaded");
    assert_eq!(c.restored, 0, "git's own markers bring no picks back");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Undo resolution of a.txt succeeded"));
    assert!(logged(&st, "Brought back the conflict in a.txt"));
    assert_eq!(redo_label(&st), "Redo resolution of a.txt");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(bytes(&dir, "a.txt"), b"one\ntheirs\n");
    assert!(!conflicted(&st, "a.txt") && staged(&st, "a.txt"));
    assert!(st.session.conflict.view.is_none(), "the next status read closes the view");
    assert!(logged(&st, "Resolved a.txt again"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_keeps_picks_saved_with_markers() {
    let dir = two_chunks("conflict-undo-picks");
    let mut st = open(&dir, "f.txt");
    let markers = bytes(&dir, "f.txt");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours });
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers));
    assert_eq!(undo_label(&st), "Undo save of f.txt with markers");
    let partial = bytes(&dir, "f.txt");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 1, side: Side::Theirs });
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    assert!(!conflicted(&st, "f.txt"));
    assert_eq!(st.session.undo.undo.len(), 2, "{:?}", st.session.undo.undo);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(bytes(&dir, "f.txt"), partial);
    assert!(conflicted(&st, "f.txt"));
    let c = view(&st);
    assert_eq!(c.picks, [ChunkPick::Sides(Choice::Ours), ChunkPick::default()]);
    assert_eq!(c.restored, 1);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(bytes(&dir, "f.txt"), markers);
    assert!(logged(&st, "Put back the earlier working copy of f.txt"));
    let c = view(&st);
    assert_eq!((c.picks.clone(), c.restored), (vec![ChunkPick::default(); 2], 0), "reloaded with no picks");
    assert_eq!(redo_label(&st), "Redo save of f.txt with markers");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(bytes(&dir, "f.txt"), partial);
    assert!(logged(&st, "Saved f.txt with markers again"));
    assert_eq!(view(&st).restored, 1, "reloaded with the saved pick");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_changed_file_refuses_and_drops_only_that_entry() {
    let (mut st, dir) = merging_after_a_commit("conflict-undo-changed");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Theirs });
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    assert_eq!(st.session.undo.undo.len(), 2, "{:?}", st.session.undo.undo);
    std::fs::write(dir.join("a.txt"), "by hand\n").unwrap();

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let (ok, text) = last_toast(&st);
    assert!(!ok && text.contains("a.txt has changed since") && text.contains("removed from the undo history"), "{text}");
    assert_eq!(bytes(&dir, "a.txt"), b"by hand\n", "left alone");
    assert!(matches!(&st.session.undo.undo[..], [Action::Commit { .. }]), "{:?}", st.session.undo.undo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn committing_the_merge_expires_the_resolution() {
    let (mut st, dir) = merging_after_a_commit("conflict-undo-committed");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Theirs });
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    st.session.draft.summary = "Merge other".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Moved { .. })), "{:?}", st.session.undo.undo);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(st.toasts.last().is_some_and(|t| t.ok), "{:?}", st.toasts.last().map(|t| &t.text));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Resolve { .. })));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let (ok, text) = last_toast(&st);
    // Undoing the merge put HEAD back where the resolution was saved, but the index no longer
    // holds the staged resolution.
    assert!(!ok && text.contains("a.txt has changed in the index since; removed from the undo history"), "{text}");
    assert!(matches!(&st.session.undo.undo[..], [Action::Commit { .. }]), "only the resolution was dropped: {:?}", st.session.undo.undo);
    assert!(matches!(&st.session.undo.redo[..], [Action::Moved { .. }]), "{:?}", st.session.undo.redo);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(st.session.undo.undo.is_empty(), "{:?}", st.session.undo.undo);
    assert!(st.session.undo.redo.iter().any(|a| matches!(a, Action::Moved { .. })), "{:?}", st.session.undo.redo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_leaves_unsaved_choices_in_another_file() {
    let dir = two_conflicts("conflict-undo-other-file");
    let mut st = open(&dir, "a.txt");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Theirs });
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflict));
    assert_eq!(view(&st).path, "b.txt", "stepped to the next conflicted file");
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours });

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(conflicted(&st, "a.txt"));
    let c = view(&st);
    assert_eq!(c.path, "b.txt");
    assert_eq!(c.picks, [ChunkPick::Sides(Choice::Ours)], "the unsaved pick is kept");
    assert!(logged(&st, "a.txt is conflicted again: open it from Conflicted Files to resolve it."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_resolution_saved_for_an_inactive_tab_goes_to_its_history() {
    let a = conflicted_repo("conflict-undo-tab-a");
    let b = scratch_repo("conflict-undo-tab-b");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    send(&mut st, ConflictMsg::ConflictToggle { chunk: 0, side: Side::Theirs });

    let effect = update(&mut st, Msg::Conflict(ConflictMsg::SaveConflict), Instant::now())
        .into_iter()
        .find(|e| matches!(e, Effect::LocalOp { .. }))
        .expect("a save");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    let reply = runner::execute(effect).expect("a result");
    update(&mut st, reply, Instant::now());

    assert!(st.session.undo.undo.is_empty(), "tab B's history is untouched");
    let parked = st.tabs[0].parked.as_ref().expect("tab A is parked");
    assert!(
        matches!(parked.session.undo.undo.last(), Some(Action::Resolve { path, .. }) if path == "a.txt"),
        "{:?}",
        parked.session.undo.undo
    );
    for d in [&a, &b] {
        let _ = std::fs::remove_dir_all(d);
    }
}
