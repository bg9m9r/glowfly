//! Headless tests for creating, renaming and deleting working-tree files: the dialogs and their
//! checks, the delete confirmations (plain, and unstage-and-delete for a staged new file), the
//! results, results for an inactive tab, and end to end against a scratch repository.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{DeleteMode, Effect, FileOp, WorkFileOk};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::WorkFileMsg;
use crate::state::files::PathForm;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{self, Deleted, DiffTarget, FileDiff, FileEntry, LoadOptions};

pub(super) fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A loaded scratch repository (one commit; a.txt modified in the working tree).
pub(super) fn loaded(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    (st, dir)
}

pub(super) fn entry(path: &str, status: char) -> FileEntry {
    FileEntry { path: path.into(), status }
}

pub(super) fn new_form(st: &AppState) -> &PathForm {
    match &st.modal {
        Some(Modal::NewFile(f)) => f,
        _ => panic!("expected the New file dialog"),
    }
}

pub(super) fn rename_form(st: &AppState) -> &PathForm {
    match &st.modal {
        Some(Modal::RenameFile { form, .. }) => form,
        _ => panic!("expected the rename dialog"),
    }
}

pub(super) fn last_log(st: &AppState) -> &str {
    st.log.last().map(|l| l.text.as_str()).unwrap_or_default()
}

pub(super) fn work_ops(fx: &[Effect]) -> Vec<FileOp> {
    fx.iter().filter_map(|e| if let Effect::WorkFile { op, .. } = e { Some(op.clone()) } else { None }).collect()
}

#[test]
fn create_checks_then_sends_the_effect() {
    let (mut st, dir) = loaded("wf-create");
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog("docs/".into()))).is_empty());
    assert_eq!(new_form(&st), &PathForm::new("docs/"));

    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("docs/a.md".into())));
    assert!(matches!(&fx[..], [Effect::WorkFile { workdir, op: FileOp::Create(p) }] if *workdir == dir && p == "docs/a.md"), "{fx:?}");
    assert_eq!(fx[0].scope(), Some(&dir));
    assert!(new_form(&st).busy);
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("docs/a.md".into()))).is_empty(), "not twice while busy");

    for (text, why) in [("../x", "outside the repository"), (".git/x", ".git folder")] {
        run(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog(String::new())));
        assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile(text.into()))).is_empty());
        let form = new_form(&st);
        assert!(matches!(&form.error, Some((t, e)) if t == text && e.contains(why)), "{:?}", form.error);
        assert!(!form.busy);
    }

    // a.txt is listed as modified.
    run(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog(String::new())));
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("a.txt".into()))).is_empty());
    assert_eq!(new_form(&st).error, Some(("a.txt".into(), "a.txt already exists.".into())));

    // Empty input does nothing.
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("  ".into()))).is_empty());

    // Without the dialog (another caller), the problem goes to the log.
    st.modal = None;
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("../x".into()))).is_empty());
    assert_eq!(last_log(&st), "Could not create ../x: That path leads outside the repository.");
    assert!(st.log.last().is_some_and(|l| !l.ok));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_normalizes_the_path() {
    let (mut st, dir) = loaded("wf-normalize");
    for text in ["a\\b.txt", " ./a//b.txt "] {
        let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile(text.into())));
        assert_eq!(work_ops(&fx), [FileOp::Create("a/b.txt".into())], "{text:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_dialog_refusals() {
    let (mut st, dir) = loaded("wf-rename-dialog");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("c.txt", '!'), entry("gone.txt", 'D'), entry("a.txt", 'M')];
    run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFileDialog("c.txt".into())));
    assert!(st.modal.is_none());
    assert!(last_log(&st).contains("conflict") && last_log(&st).contains("c.txt"));
    run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFileDialog("gone.txt".into())));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "gone.txt is not in your working tree.");

    run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFileDialog("a.txt".into())));
    assert!(matches!(&st.modal, Some(Modal::RenameFile { old, form }) if old == "a.txt" && *form == PathForm::new("a.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_checks_the_target() {
    let (mut st, dir) = loaded("wf-rename");
    st.snap.as_mut().unwrap().unstaged = vec![entry("a.txt", 'M'), entry("b.txt", '?')];
    run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFileDialog("a.txt".into())));

    assert!(
        run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "a.txt".into(), stage: false })).is_empty(),
        "unchanged"
    );
    assert!(rename_form(&st).error.is_none());

    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "b.txt".into(), stage: false })).is_empty());
    assert_eq!(rename_form(&st).error, Some(("b.txt".into(), "b.txt already exists.".into())));

    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "A.txt".into(), stage: false }));
    assert_eq!(work_ops(&fx), [FileOp::Rename { from: "a.txt".into(), to: "A.txt".into(), stage: false }]);
    assert!(rename_form(&st).busy);
    let _ = std::fs::remove_dir_all(&dir);
}

pub(super) fn confirm(st: &AppState) -> (&str, &str, &str, &Msg) {
    match &st.modal {
        Some(Modal::Confirm { title, body, button, then }) => (title, body, button, then),
        _ => panic!("expected the confirmation"),
    }
}

#[test]
fn delete_asks_first() {
    let (mut st, dir) = loaded("wf-delete-ask");
    // The permanent texts; the trash ones are in work_files_trash_tests.
    st.settings.delete_to_trash = false;
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("new.txt", '?'), entry("a.txt", 'M'), entry("c.txt", '!')];
    snap.staged = vec![entry("s.txt", 'M')];

    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false })).is_empty());
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Delete file", "Delete"));
    assert!(body.contains("isn't tracked"), "{body}");
    assert!(
        matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::Delete { path, mode: DeleteMode::Permanent })) if path == "new.txt")
    );

    run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "a.txt".into(), confirmed: false }));
    assert!(confirm(&st).1.contains("changes to a.txt are lost"), "{}", confirm(&st).1);
    run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "s.txt".into(), confirmed: false }));
    assert!(confirm(&st).1.contains("brings it back as it is now"), "{}", confirm(&st).1);

    st.modal = None;
    run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "c.txt".into(), confirmed: false }));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "Resolve the conflict in c.txt before deleting it.");

    run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false }));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [FileOp::Delete { path: "new.txt".into(), mode: DeleteMode::Permanent }]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_staged_new_file_asks_to_unstage() {
    let (mut st, dir) = loaded("wf-delete-new");
    st.settings.delete_to_trash = false;
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = Vec::new();
    snap.staged = vec![entry("s.txt", 'A')];

    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "s.txt".into(), confirmed: false })).is_empty());
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Delete new file", "Unstage and delete"));
    assert!(body.contains("s.txt") && body.contains("staging area"), "{body}");
    assert!(!body.contains("unstaged edits"), "{body}");
    let op = FileOp::UnstageDelete { path: "s.txt".into(), mode: DeleteMode::Permanent };
    assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(o)) if *o == op));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [op]);
    assert_eq!(fx[0].scope(), Some(&dir));

    // Further edits to the new file aren't staged: they go too.
    st.snap.as_mut().unwrap().unstaged = vec![entry("s.txt", 'M')];
    run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "s.txt".into(), confirmed: false }));
    let (title, body, ..) = confirm(&st);
    assert_eq!(title, "Delete new file");
    assert!(body.contains("unstaged edits"), "{body}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_delete_results() {
    let (mut st, dir) = loaded("wf-unstage-delete-done");
    let op = || FileOp::UnstageDelete { path: "s.txt".into(), mode: DeleteMode::Permanent };
    st.diff = Some((DiffTarget::Staged("s.txt".into()), empty_diff().into()));
    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Ok(WorkFileOk::Deleted(Deleted::Permanently)) }));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Deleted s.txt and removed it from the staging area."));
    assert!(st.diff.is_none(), "the staged diff of the deleted file is closed");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");

    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Err("boom".into()) }));
    assert_eq!(last_log(&st), "Could not delete s.txt: boom");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_result_closes_or_shows_the_error() {
    let (mut st, dir) = loaded("wf-create-done");
    let create = || FileOp::Create("docs/a.md".into());
    run(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog("docs/a.md".into())));
    run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("docs/a.md".into())));
    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: create(), result: Ok(WorkFileOk::Written { ignored: false }) }));
    assert!(st.modal.is_none());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Created docs/a.md."));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");
    finish(&mut st, fx);

    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: create(), result: Ok(WorkFileOk::Written { ignored: true }) }));
    finish(&mut st, fx);
    assert!(st.toasts.last().unwrap().text.contains("Git ignores it"));

    run(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog("docs/a.md".into())));
    run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("docs/a.md".into())));
    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: create(), result: Err("docs/a.md already exists".into()) }));
    let form = new_form(&st);
    assert!(!form.busy);
    assert_eq!(form.error, Some(("docs/a.md".into(), "docs/a.md already exists".into())));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })));

    st.modal = None;
    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: create(), result: Err("boom".into()) }));
    assert_eq!(last_log(&st), "Could not create docs/a.md: boom");
    let _ = std::fs::remove_dir_all(&dir);
}

pub(super) fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

#[test]
fn rename_or_delete_closes_the_old_paths_diff() {
    let (mut st, dir) = loaded("wf-close-diff");
    for (op, ok) in [
        (FileOp::Delete { path: "a.txt".into(), mode: DeleteMode::Trash }, WorkFileOk::Deleted(Deleted::Trashed)),
        (FileOp::Rename { from: "a.txt".into(), to: "b.txt".into(), stage: false }, WorkFileOk::Written { ignored: false }),
    ] {
        st.diff = Some((DiffTarget::Unstaged("a.txt".into()), empty_diff().into()));
        run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op.clone(), result: Ok(ok) }));
        assert!(st.diff.is_none(), "{op:?}");

        st.diff = Some((DiffTarget::Unstaged("other.txt".into()), empty_diff().into()));
        run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op, result: Ok(ok) }));
        assert!(st.diff.is_some());
    }
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Renamed a.txt to b.txt."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn result_for_inactive_tab() {
    let (mut st, dir) = loaded("wf-scoped");
    run(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog(String::new())));
    run(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("x.txt".into())));
    assert!(new_form(&st).busy);
    let msg = Msg::WorkFile(WorkFileMsg::WorkFileDone { op: FileOp::Create("x.txt".into()), result: Err("in the way".into()) });
    let fx = run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) });
    assert!(fx.is_empty());
    assert!(st.modal.is_none(), "the waiting dialog is closed");
    assert_eq!(last_log(&st), "Could not create x.txt: in the way");

    for (result, want) in [
        (Ok(WorkFileOk::Deleted(Deleted::Permanently)), "Deleted s.txt and removed it from the staging area."),
        (Err("in the way".to_owned()), "Could not delete s.txt: in the way"),
    ] {
        let ok = result.is_ok();
        let msg = Msg::WorkFile(WorkFileMsg::WorkFileDone {
            op: FileOp::UnstageDelete { path: "s.txt".into(), mode: DeleteMode::Permanent },
            result,
        });
        assert!(run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) }).is_empty());
        let text = if ok { st.toasts.last().map(|t| t.text.as_str()) } else { st.log.last().map(|l| l.text.as_str()) };
        assert_eq!(text, Some(want));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_create_delete_rename() {
    let dir = scratch_repo("wf-e2e");
    let mut st = state();
    st.settings.delete_to_trash = false;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog(String::new())));
    drive(&mut st, Msg::WorkFile(WorkFileMsg::CreateFile("notes/todo.txt".into())));
    assert!(dir.join("notes/todo.txt").is_file());
    assert!(st.modal.is_none());
    let unstaged = &st.snap.as_ref().unwrap().unstaged;
    assert!(unstaged.contains(&entry("notes/todo.txt", '?')), "{unstaged:?}");

    drive(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "notes/todo.txt".into(), confirmed: true }));
    assert!(!dir.join("notes").exists());
    assert!(st.toasts.last().is_some_and(|t| t.text == "Deleted notes/todo.txt."));

    drive(&mut st, Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "b.txt".into(), stage: false }));
    let unstaged = &st.snap.as_ref().unwrap().unstaged;
    assert!(unstaged.contains(&entry("a.txt", 'D')) && unstaged.contains(&entry("b.txt", '?')), "{unstaged:?}");
    assert!(st.snap.as_ref().unwrap().staged.is_empty(), "nothing staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_unstage_and_delete() {
    let dir = scratch_repo("wf-e2e-unstage-delete");
    std::fs::write(
        dir.join("new.txt"),
        "new
",
    )
    .unwrap();
    git(&dir, &["add", "new.txt"]);
    let mut st = state();
    st.settings.delete_to_trash = false;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.as_ref().unwrap().staged.contains(&entry("new.txt", 'A')));

    drive(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false }));
    assert!(matches!(&st.modal, Some(Modal::Confirm { title, .. }) if title == "Delete new file"));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(!dir.join("new.txt").exists());
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.iter().chain(&snap.unstaged).all(|f| f.path != "new.txt"), "{:?} {:?}", snap.staged, snap.unstaged);
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Deleted new.txt and removed it from the staging area."));
    let _ = std::fs::remove_dir_all(&dir);
}
