//! Headless tests for creating, renaming and deleting working-tree files: the dialogs and their
//! checks, the delete confirmation, the results, results for an inactive tab, and end to end
//! against a scratch repository.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, FileOp};
use crate::msg::Msg;
use crate::state::files::PathForm;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{self, DiffTarget, FileDiff, FileEntry, LoadOptions};

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A loaded scratch repository (one commit; a.txt modified in the working tree).
fn loaded(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

fn entry(path: &str, status: char) -> FileEntry {
    FileEntry { path: path.into(), status }
}

fn new_form(st: &AppState) -> &PathForm {
    match &st.modal {
        Some(Modal::NewFile(f)) => f,
        _ => panic!("expected the New file dialog"),
    }
}

fn rename_form(st: &AppState) -> &PathForm {
    match &st.modal {
        Some(Modal::RenameFile { form, .. }) => form,
        _ => panic!("expected the rename dialog"),
    }
}

fn last_log(st: &AppState) -> &str {
    st.log.last().map(|l| l.text.as_str()).unwrap_or_default()
}

fn work_ops(fx: &[Effect]) -> Vec<FileOp> {
    fx.iter().filter_map(|e| if let Effect::WorkFile { op, .. } = e { Some(op.clone()) } else { None }).collect()
}

#[test]
fn create_checks_then_sends_the_effect() {
    let (mut st, dir) = loaded("wf-create");
    assert!(run(&mut st, Msg::NewFileDialog("docs/".into())).is_empty());
    assert_eq!(new_form(&st), &PathForm::new("docs/"));

    let fx = run(&mut st, Msg::CreateFile("docs/a.md".into()));
    assert!(matches!(&fx[..], [Effect::WorkFile { workdir, op: FileOp::Create(p) }] if *workdir == dir && p == "docs/a.md"), "{fx:?}");
    assert_eq!(fx[0].scope(), Some(&dir));
    assert!(new_form(&st).busy);
    assert!(run(&mut st, Msg::CreateFile("docs/a.md".into())).is_empty(), "not twice while busy");

    for (text, why) in [("../x", "outside the repository"), (".git/x", ".git folder")] {
        run(&mut st, Msg::NewFileDialog(String::new()));
        assert!(run(&mut st, Msg::CreateFile(text.into())).is_empty());
        let form = new_form(&st);
        assert!(matches!(&form.error, Some((t, e)) if t == text && e.contains(why)), "{:?}", form.error);
        assert!(!form.busy);
    }

    // a.txt is listed as modified.
    run(&mut st, Msg::NewFileDialog(String::new()));
    assert!(run(&mut st, Msg::CreateFile("a.txt".into())).is_empty());
    assert_eq!(new_form(&st).error, Some(("a.txt".into(), "a.txt already exists.".into())));

    // Empty input does nothing.
    assert!(run(&mut st, Msg::CreateFile("  ".into())).is_empty());

    // Without the dialog (another caller), the problem goes to the log.
    st.modal = None;
    assert!(run(&mut st, Msg::CreateFile("../x".into())).is_empty());
    assert_eq!(last_log(&st), "Could not create ../x: That path leads outside the repository.");
    assert!(st.log.last().is_some_and(|l| !l.ok));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_normalizes_the_path() {
    let (mut st, dir) = loaded("wf-normalize");
    for text in ["a\\b.txt", " ./a//b.txt "] {
        let fx = run(&mut st, Msg::CreateFile(text.into()));
        assert_eq!(work_ops(&fx), [FileOp::Create("a/b.txt".into())], "{text:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_dialog_refusals() {
    let (mut st, dir) = loaded("wf-rename-dialog");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("c.txt", '!'), entry("gone.txt", 'D'), entry("a.txt", 'M')];
    run(&mut st, Msg::RenameFileDialog("c.txt".into()));
    assert!(st.modal.is_none());
    assert!(last_log(&st).contains("conflict") && last_log(&st).contains("c.txt"));
    run(&mut st, Msg::RenameFileDialog("gone.txt".into()));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "gone.txt is not in your working tree.");

    run(&mut st, Msg::RenameFileDialog("a.txt".into()));
    assert!(matches!(&st.modal, Some(Modal::RenameFile { old, form }) if old == "a.txt" && *form == PathForm::new("a.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_checks_the_target() {
    let (mut st, dir) = loaded("wf-rename");
    st.snap.as_mut().unwrap().unstaged = vec![entry("a.txt", 'M'), entry("b.txt", '?')];
    run(&mut st, Msg::RenameFileDialog("a.txt".into()));

    assert!(run(&mut st, Msg::RenameFile { from: "a.txt".into(), to: "a.txt".into() }).is_empty(), "unchanged");
    assert!(rename_form(&st).error.is_none());

    assert!(run(&mut st, Msg::RenameFile { from: "a.txt".into(), to: "b.txt".into() }).is_empty());
    assert_eq!(rename_form(&st).error, Some(("b.txt".into(), "b.txt already exists.".into())));

    let fx = run(&mut st, Msg::RenameFile { from: "a.txt".into(), to: "A.txt".into() });
    assert_eq!(work_ops(&fx), [FileOp::Rename { from: "a.txt".into(), to: "A.txt".into() }]);
    assert!(rename_form(&st).busy);
    let _ = std::fs::remove_dir_all(&dir);
}

fn confirm(st: &AppState) -> (&str, &str, &str, &Msg) {
    match &st.modal {
        Some(Modal::Confirm { title, body, button, then }) => (title, body, button, then),
        _ => panic!("expected the confirmation"),
    }
}

#[test]
fn delete_asks_first() {
    let (mut st, dir) = loaded("wf-delete-ask");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("new.txt", '?'), entry("a.txt", 'M'), entry("c.txt", '!')];
    snap.staged = vec![entry("s.txt", 'A')];

    assert!(run(&mut st, Msg::DeleteFile { path: "new.txt".into(), confirmed: false }).is_empty());
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Delete file", "Delete"));
    assert!(body.contains("isn't tracked"), "{body}");
    assert!(matches!(then, Msg::DeleteFile { path, confirmed: true } if path == "new.txt"));

    run(&mut st, Msg::DeleteFile { path: "a.txt".into(), confirmed: false });
    assert!(confirm(&st).1.contains("changes to a.txt are lost"), "{}", confirm(&st).1);
    run(&mut st, Msg::DeleteFile { path: "s.txt".into(), confirmed: false });
    assert!(confirm(&st).1.contains("brings it back as it is now"), "{}", confirm(&st).1);

    st.modal = None;
    run(&mut st, Msg::DeleteFile { path: "c.txt".into(), confirmed: false });
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "Resolve the conflict in c.txt before deleting it.");

    run(&mut st, Msg::DeleteFile { path: "new.txt".into(), confirmed: false });
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [FileOp::Delete("new.txt".into())]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_result_closes_or_shows_the_error() {
    let (mut st, dir) = loaded("wf-create-done");
    let create = || FileOp::Create("docs/a.md".into());
    run(&mut st, Msg::NewFileDialog("docs/a.md".into()));
    run(&mut st, Msg::CreateFile("docs/a.md".into()));
    let fx = run(&mut st, Msg::WorkFileDone { op: create(), result: Ok(false) });
    assert!(st.modal.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Created docs/a.md."));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");

    run(&mut st, Msg::WorkFileDone { op: create(), result: Ok(true) });
    assert!(st.toasts.last().unwrap().text.contains("Git ignores it"));

    run(&mut st, Msg::NewFileDialog("docs/a.md".into()));
    run(&mut st, Msg::CreateFile("docs/a.md".into()));
    let fx = run(&mut st, Msg::WorkFileDone { op: create(), result: Err("docs/a.md already exists".into()) });
    let form = new_form(&st);
    assert!(!form.busy);
    assert_eq!(form.error, Some(("docs/a.md".into(), "docs/a.md already exists".into())));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })));

    st.modal = None;
    run(&mut st, Msg::WorkFileDone { op: create(), result: Err("boom".into()) });
    assert_eq!(last_log(&st), "Could not create docs/a.md: boom");
    let _ = std::fs::remove_dir_all(&dir);
}

fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

#[test]
fn rename_or_delete_closes_the_old_paths_diff() {
    let (mut st, dir) = loaded("wf-close-diff");
    for op in [FileOp::Delete("a.txt".into()), FileOp::Rename { from: "a.txt".into(), to: "b.txt".into() }] {
        st.diff = Some((DiffTarget::Unstaged("a.txt".into()), empty_diff()));
        run(&mut st, Msg::WorkFileDone { op: op.clone(), result: Ok(false) });
        assert!(st.diff.is_none(), "{op:?}");

        st.diff = Some((DiffTarget::Unstaged("other.txt".into()), empty_diff()));
        run(&mut st, Msg::WorkFileDone { op, result: Ok(false) });
        assert!(st.diff.is_some());
    }
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Renamed a.txt to b.txt."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn result_for_inactive_tab() {
    let (mut st, dir) = loaded("wf-scoped");
    run(&mut st, Msg::NewFileDialog(String::new()));
    run(&mut st, Msg::CreateFile("x.txt".into()));
    assert!(new_form(&st).busy);
    let msg = Msg::WorkFileDone { op: FileOp::Create("x.txt".into()), result: Err("in the way".into()) };
    let fx = run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) });
    assert!(fx.is_empty());
    assert!(st.modal.is_none(), "the waiting dialog is closed");
    assert_eq!(last_log(&st), "Could not create x.txt: in the way");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_create_delete_rename() {
    let dir = scratch_repo("wf-e2e");
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    drive(&mut st, Msg::NewFileDialog(String::new()));
    drive(&mut st, Msg::CreateFile("notes/todo.txt".into()));
    assert!(dir.join("notes/todo.txt").is_file());
    assert!(st.modal.is_none());
    let unstaged = &st.snap.as_ref().unwrap().unstaged;
    assert!(unstaged.contains(&entry("notes/todo.txt", '?')), "{unstaged:?}");

    drive(&mut st, Msg::DeleteFile { path: "notes/todo.txt".into(), confirmed: true });
    assert!(!dir.join("notes").exists());
    assert!(st.toasts.last().is_some_and(|t| t.text == "Deleted notes/todo.txt."));

    drive(&mut st, Msg::RenameFile { from: "a.txt".into(), to: "b.txt".into() });
    let unstaged = &st.snap.as_ref().unwrap().unstaged;
    assert!(unstaged.contains(&entry("a.txt", 'D')) && unstaged.contains(&entry("b.txt", '?')), "{unstaged:?}");
    assert!(st.snap.as_ref().unwrap().staged.is_empty(), "nothing staged");
    let _ = std::fs::remove_dir_all(&dir);
}
