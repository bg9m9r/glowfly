//! Headless tests for discarding an untracked file: the confirmation's wording with the Recycle
//! Bin / Trash on and off, the mode fixed when it opens, refusals for paths that aren't untracked,
//! the permanent fallback when the trash refuses the file, the toasts and failures, results for an
//! inactive tab, and end to end with the test build's stand-in trash (never the real one).
//! Discarding a tracked file still reverts it to the index version.

use super::tests::{drive, scratch_repo, state};
use super::work_files_tests::{confirm, empty_diff, entry, last_log, loaded, run, work_ops};
use crate::effect::{DeleteMode, Effect, FileOp, LocalOp, WorkFileOk};
use crate::msg::Msg;
use crate::state::files::TRASH;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use trellis_core::repo::{Deleted, DiffTarget};

fn discard(path: &str, mode: DeleteMode) -> FileOp {
    FileOp::Discard { path: path.into(), mode }
}

/// A loaded state whose snapshot lists `u.txt` as untracked.
fn with_untracked(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = loaded(name);
    st.snap.as_mut().unwrap().unstaged = vec![entry("u.txt", '?')];
    (st, dir)
}

/// Opens the confirmation for discarding `path` and returns its body, button and the op it runs.
fn ask(st: &mut AppState, path: &str) -> (String, String, FileOp) {
    assert!(run(st, Msg::DiscardUntracked(path.into())).is_empty());
    let (title, body, button, then) = confirm(st);
    assert_eq!(title, "Discard changes");
    let Msg::DeleteConfirmed(op) = then else { panic!("expected DeleteConfirmed for {path}") };
    (body.to_owned(), button.to_owned(), op.clone())
}

#[test]
fn discard_confirm_trash() {
    let (mut st, dir) = with_untracked("wfd-trash");
    assert!(st.settings.delete_to_trash);
    let (body, button, op) = ask(&mut st, "u.txt");
    assert_eq!(body, format!("u.txt isn't tracked by git. Discarding it moves it to the {TRASH}, so you can restore it from there."));
    assert_eq!(button, format!("Move to {TRASH}"));
    assert_eq!(op, discard("u.txt", DeleteMode::Trash));

    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [discard("u.txt", DeleteMode::Trash)]);
    assert_eq!(fx[0].scope(), Some(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_confirm_trash_off() {
    let (mut st, dir) = with_untracked("wfd-trash-off");
    st.settings.delete_to_trash = false;
    let (body, button, op) = ask(&mut st, "u.txt");
    assert_eq!(body, "Delete untracked file u.txt? This cannot be undone.");
    assert!(!body.contains(TRASH));
    assert_eq!(button, "Discard");
    assert_eq!(op, discard("u.txt", DeleteMode::Permanent));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_mode_fixed_at_ask_time() {
    let (mut st, dir) = with_untracked("wfd-fixed");
    ask(&mut st, "u.txt");
    st.settings.delete_to_trash = false;
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [discard("u.txt", DeleteMode::Trash)]);

    ask(&mut st, "u.txt");
    st.settings.delete_to_trash = true;
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [discard("u.txt", DeleteMode::Permanent)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_refuses_non_untracked() {
    let (mut st, dir) = loaded("wfd-refuse");
    let want = "u.txt is no longer an untracked file; refresh and try again.";
    for listed in [vec![entry("u.txt", 'M')], Vec::new(), vec![entry("other.txt", '?')]] {
        st.snap.as_mut().unwrap().unstaged = listed.clone();
        st.log.clear();
        assert!(run(&mut st, Msg::DiscardUntracked("u.txt".into())).is_empty(), "{listed:?}");
        assert!(st.modal.is_none(), "{listed:?}");
        assert_eq!(last_log(&st), want);
        assert!(st.log.last().is_some_and(|l| !l.ok));
    }
    st.snap = None;
    assert!(run(&mut st, Msg::DiscardUntracked("u.txt".into())).is_empty());
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_trash_refused_offers_permanent() {
    let (mut st, dir) = loaded("wfd-refused");
    let op = discard("u.txt", DeleteMode::Trash);
    let fx = run(&mut st, Msg::TrashRefused { op, reason: "this drive has no Recycle Bin".into() });
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Delete permanently?", "Delete permanently"));
    assert_eq!(
        body,
        format!(
            "u.txt couldn't be moved to the {TRASH}: this drive has no Recycle Bin.\n\nDelete it permanently instead? This can't be undone."
        )
    );
    assert!(matches!(then, Msg::DeleteConfirmed(op) if *op == discard("u.txt", DeleteMode::PermanentNoTrash)));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [discard("u.txt", DeleteMode::PermanentNoTrash)]);

    // Cancelling changes nothing.
    run(&mut st, Msg::TrashRefused { op: discard("u.txt", DeleteMode::Trash), reason: "no bin".into() });
    assert!(run(&mut st, Msg::CloseModal).is_empty());
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_toasts() {
    let (mut st, dir) = loaded("wfd-toasts");
    let cases = [
        (DeleteMode::Trash, Deleted::Trashed, format!("Moved u.txt to the {TRASH}.")),
        (DeleteMode::Permanent, Deleted::Permanently, "Deleted u.txt.".to_owned()),
        (DeleteMode::PermanentNoTrash, Deleted::Permanently, format!("Deleted u.txt permanently; the {TRASH} couldn't take it.")),
        (
            DeleteMode::Trash,
            Deleted::LinkRemoved,
            format!("Deleted the symbolic link u.txt. Links don't go to the {TRASH}; what it points to is untouched."),
        ),
    ];
    for (mode, how, want) in cases {
        run(&mut st, Msg::WorkFileDone { op: discard("u.txt", mode), result: Ok(WorkFileOk::Deleted(how)) });
        assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some(want.as_str()), "{mode:?} {how:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_failure_logs_and_closes_diff() {
    let (mut st, dir) = loaded("wfd-fail");
    st.diff = Some((DiffTarget::Unstaged("u.txt".into()), empty_diff()));
    let ok = Ok(WorkFileOk::Deleted(Deleted::Trashed));
    let fx = run(&mut st, Msg::WorkFileDone { op: discard("u.txt", DeleteMode::Trash), result: ok });
    assert!(st.diff.is_none());
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");

    st.diff = Some((DiffTarget::Unstaged("other.txt".into()), empty_diff()));
    run(&mut st, Msg::WorkFileDone { op: discard("u.txt", DeleteMode::Trash), result: Err("boom".into()) });
    assert_eq!(last_log(&st), "Could not discard u.txt: boom");
    assert!(st.log.last().is_some_and(|l| !l.ok));
    assert!(st.diff.is_some(), "another file's diff stays open");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_inactive_tab() {
    let (mut st, dir) = loaded("wfd-scoped");
    let elsewhere = || PathBuf::from("C:/elsewhere");
    let msg = Msg::TrashRefused { op: discard("u.txt", DeleteMode::Trash), reason: "no bin".into() };
    assert!(run(&mut st, Msg::Scoped { workdir: elsewhere(), msg: Box::new(msg) }).is_empty());
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "Did not delete u.txt: no bin.");

    let msg = Msg::WorkFileDone { op: discard("u.txt", DeleteMode::Trash), result: Ok(WorkFileOk::Deleted(Deleted::Trashed)) };
    assert!(run(&mut st, Msg::Scoped { workdir: elsewhere(), msg: Box::new(msg) }).is_empty());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some(format!("Moved u.txt to the {TRASH}.").as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_confirmed_accepts_discard() {
    let (mut st, dir) = loaded("wfd-confirmed");
    let fx = run(&mut st, Msg::DeleteConfirmed(discard("a.txt", DeleteMode::Trash)));
    assert_eq!(work_ops(&fx), [discard("a.txt", DeleteMode::Trash)]);
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].scope(), Some(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tracked_discard_unchanged() {
    let (mut st, dir) = loaded("wfd-tracked");
    let fx = run(&mut st, Msg::DiscardFile("f.txt".into()));
    assert!(matches!(&fx[..], [Effect::LocalOp { workdir, op: LocalOp::DiscardFile(p) }] if *workdir == dir && p == "f.txt"), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Discards the untracked u.txt in a fresh scratch repository, the trash on or off; returns the
/// repository, the state and u.txt's unique contents.
fn discard_end_to_end(name: &str, to_trash: bool) -> (PathBuf, AppState, String) {
    let dir = scratch_repo(name);
    let contents = format!("discard me {}\n", dir.display());
    std::fs::write(dir.join("u.txt"), &contents).unwrap();
    let mut st = state();
    st.settings.delete_to_trash = to_trash;
    drive(&mut st, Msg::Open(dir.clone()));
    assert!(st.snap.as_ref().unwrap().unstaged.contains(&entry("u.txt", '?')));
    drive(&mut st, Msg::DiscardUntracked("u.txt".into()));
    assert!(matches!(&st.modal, Some(Modal::Confirm { .. })));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(!dir.join("u.txt").exists());
    assert!(st.snap.as_ref().unwrap().unstaged.iter().all(|f| f.path != "u.txt"));
    (dir, st, contents)
}

/// Whether the test build's stand-in bin holds u.txt with `contents`.
fn in_test_bin(contents: &str) -> bool {
    std::fs::read_dir(crate::runner::files::test_bin())
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().ends_with("-u.txt") && std::fs::read_to_string(e.path()).is_ok_and(|t| t == contents))
}

#[test]
fn end_to_end_discard_trash() {
    let (dir, st, contents) = discard_end_to_end("wfd-e2e-trash", true);
    assert!(in_test_bin(&contents), "u.txt is in the stand-in bin");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some(format!("Moved u.txt to the {TRASH}.").as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_discard_permanent() {
    let (dir, st, contents) = discard_end_to_end("wfd-e2e-perm", false);
    assert!(!in_test_bin(&contents));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Deleted u.txt."));
    let _ = std::fs::remove_dir_all(&dir);
}
