//! Headless tests for undoing conflict changes made without the editor: Mark resolved from the
//! WIP panel and Mark conflicted from the palette through Edit > Undo, a whole binary side
//! taken in the conflict view through its Resolved toast's Undo (the conflict stages and the
//! file's bytes come back exactly), the Saved-with-markers toast's Undo, and an entry the merge
//! outlived refused and dropped alone.

use super::conflict_files_tests::both_resolved;
use super::conflict_tests::conflicted_repo;
use super::conflict_undo_tests::two_chunks;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::msg::{ConflictMsg, JobMsg, Msg, PaletteMsg, RepoMsg};
use crate::state::AppState;
use crate::state::palette::PaletteCommand as C;
use glowfly_core::gitcli::{args, run};
use glowfly_core::repo::Side;
use glowfly_core::undo::{Action, ConflictChange};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The index entries of `f`, as `git ls-files -s` lists them (mode, blob and stage).
pub(super) fn stages(dir: &Path, f: &str) -> String {
    let (ok, out) = run(dir, &args(&["ls-files", "-s", "--", f]));
    assert!(ok, "{out}");
    out
}

fn conflicted(st: &AppState, path: &str) -> bool {
    st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.status == '!' && f.path == path))
}

pub(super) fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    st
}

fn undo_label(st: &AppState) -> String {
    st.session.undo.undo_entry(st.settings.lang(), true).0
}

/// Clicks the newest toast's button, which must be Undo.
fn click_undo(st: &mut AppState) {
    let t = st.toasts.last().expect("a toast");
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("Undo"), "{:?}", t.text);
    let id = t.id;
    drive(st, Msg::ToastAction(id));
}

/// A repository mid-merge where logo.bin, a binary file, conflicts.
pub(super) fn binary_conflict(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    let put = |s: &str| std::fs::write(dir.join("logo.bin"), format!("\0\x01{s}\0")).unwrap();
    put("base");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    put("theirs");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    put("ours");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0, "merge should conflict");
    dir
}

#[test]
fn mark_resolved_from_the_wip_panel_is_undone_and_redone() {
    let dir = conflicted_repo("cmu-mark-resolved");
    let conflict = stages(&dir, "a.txt");
    let markers = std::fs::read(dir.join("a.txt")).unwrap();
    let mut st = opened(&dir);
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(vec!["a.txt".into()])));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(!conflicted(&st, "a.txt"));
    assert_eq!(undo_label(&st), "Undo marking a.txt resolved");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(conflicted(&st, "a.txt"));
    assert_eq!(stages(&dir, "a.txt"), conflict, "stages 1, 2 and 3 exactly as they were");
    assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), markers, "the working file is untouched");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Undo marking a.txt resolved succeeded"), "{:?}", st.toasts.last().map(|t| &t.text));
    assert!(st.session.conflict.view.as_ref().is_some_and(|c| c.path == "a.txt"), "the conflict opens again");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(!conflicted(&st, "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_conflicted_from_the_palette_is_undone() {
    let (mut st, dir) = both_resolved("cmu-mark-conflicted");
    std::fs::write(dir.join("a.txt"), "my resolution\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let resolved = stages(&dir, "a.txt");
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::MarkConflicted("a.txt".into()))));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(conflicted(&st, "a.txt"));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Conflicts { change: ConflictChange::MarkConflicted, .. })));
    assert_eq!(undo_label(&st), "Undo marking a.txt conflicted");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(!conflicted(&st, "a.txt"));
    assert_eq!(stages(&dir, "a.txt"), resolved);
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "my resolution\n", "the edit git overwrote is back");
    assert!(st.session.conflict_files.resolved.iter().any(|r| r.entry.path == "a.txt"), "it can be marked conflicted again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_resolved_toast_undoes_a_whole_binary_side() {
    let dir = binary_conflict("cmu-binary");
    let conflict = stages(&dir, "logo.bin");
    let ours = std::fs::read(dir.join("logo.bin")).unwrap();
    let mut st = opened(&dir);
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("logo.bin".into())));
    drive(&mut st, Msg::Conflict(ConflictMsg::ResolveWithSide { path: "logo.bin".into(), side: Side::Theirs }));
    assert_eq!(std::fs::read(dir.join("logo.bin")).unwrap(), b"\0\x01theirs\0");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Resolved logo.bin"));
    assert_eq!(undo_label(&st), "Undo taking their version of logo.bin");

    click_undo(&mut st);
    assert!(conflicted(&st, "logo.bin"));
    assert_eq!(stages(&dir, "logo.bin"), conflict);
    assert_eq!(std::fs::read(dir.join("logo.bin")).unwrap(), ours);
    assert!(st.session.conflict.view.as_ref().is_some_and(|c| c.path == "logo.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_saved_with_markers_toast_undoes_the_save() {
    let dir = two_chunks("cmu-markers");
    let markers = std::fs::read(dir.join("f.txt")).unwrap();
    let mut st = opened(&dir);
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("f.txt".into())));
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours }), Instant::now());
    drive(&mut st, Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers));
    assert_ne!(std::fs::read(dir.join("f.txt")).unwrap(), markers);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Saved f.txt with 1 conflict left (still conflicted)"));

    click_undo(&mut st);
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), markers);
    assert!(conflicted(&st, "f.txt"));
    assert_eq!(st.session.undo.redo_entry(st.settings.lang(), true).0, "Redo save of f.txt with markers");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_undo_the_merge_outlived_is_refused_and_dropped_alone() {
    let dir = conflicted_repo("cmu-aborted");
    let mut st = opened(&dir);
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(vec!["a.txt".into()])));
    drive(&mut st, Msg::ConfirmAccepted);
    git(&dir, &["merge", "--abort"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let head = stages(&dir, "a.txt");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let t = st.toasts.last().expect("a toast");
    assert!(!t.ok && t.text.contains("The merge that was in progress during the marking a.txt resolved is over"), "{}", t.text);
    assert_eq!(stages(&dir, "a.txt"), head, "nothing was put back");
    assert!(st.session.undo.undo.is_empty(), "{:?}", st.session.undo.undo);
    let _ = std::fs::remove_dir_all(&dir);
}
