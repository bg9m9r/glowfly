//! Headless tests for deleting a staged rename's new path (unstage and delete, putting the
//! rename's source back in the index) and for unstaging a staged new file or rename whose
//! working copy is already gone: the confirmations, the results, and end to end against a
//! scratch repository.

use super::tests::{drive, git, scratch_repo, state};
use super::work_files_tests::{confirm, empty_diff, entry, last_log, loaded, run, work_ops};
use crate::effect::{DeleteMode, Effect, FileOp, WorkFileOk};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::WorkFileMsg;
use crate::state::Modal;
use glowfly_core::repo::{DiffTarget, FileEntry};
use std::path::PathBuf;

#[test]
fn delete_staged_rename_asks_to_unstage_the_rename() {
    let (mut st, dir) = loaded("wfu-rename");
    // The permanent texts; the trash ones are in work_files_trash_tests.
    st.settings.delete_to_trash = false;
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = Vec::new();
    snap.staged = vec![entry("new.txt", 'R')];

    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false })).is_empty());
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Delete renamed file", "Unstage and delete"));
    assert!(body.contains("new.txt") && body.contains("rename"), "{body}");
    assert!(!body.contains("unstaged edits"), "{body}");
    let op = FileOp::UnstageDelete { path: "new.txt".into(), mode: DeleteMode::Permanent };
    assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(o)) if *o == op));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [op]);
    assert_eq!(fx[0].scope(), Some(&dir));

    st.snap.as_mut().unwrap().unstaged = vec![entry("new.txt", 'M')];
    run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false }));
    let (title, body, ..) = confirm(&st);
    assert_eq!(title, "Delete renamed file");
    // Undo keeps the unstaged edits: they are the bytes it puts back.
    assert!(body.ends_with("brings back the file and the staged rename.") && !body.contains("lost"), "{body}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_gone_staged_new_file_asks_to_unstage() {
    let (mut st, dir) = loaded("wfu-gone");
    for (status, mentions) in [('A', "new file"), ('R', "rename")] {
        let snap = st.snap.as_mut().unwrap();
        snap.staged = vec![entry("s.txt", status)];
        snap.unstaged = vec![entry("s.txt", 'D')];
        st.modal = None;

        assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "s.txt".into(), confirmed: false })).is_empty());
        let (title, body, button, then) = confirm(&st);
        assert_eq!((title, button), ("Unstage deleted file", "Unstage"), "{status}");
        assert!(body.contains("s.txt") && body.contains(mentions), "{body}");
        assert!(!body.contains("unstaged edits"), "{body}");
        assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::UnstageGone(p))) if p == "s.txt"));
        let fx = run(&mut st, Msg::ConfirmAccepted);
        assert_eq!(work_ops(&fx), [FileOp::UnstageGone("s.txt".into())]);
        assert_eq!(fx[0].scope(), Some(&dir));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gone_file_not_staged_new_is_still_refused() {
    let (mut st, dir) = loaded("wfu-refused");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("a.txt", 'D'), entry("m.txt", 'D')];
    snap.staged = vec![entry("m.txt", 'M')];
    for path in ["a.txt", "m.txt"] {
        assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: path.into(), confirmed: false })).is_empty());
        assert!(st.modal.is_none(), "{path}");
        assert_eq!(last_log(&st), format!("{path} is not in your working tree."));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_gone_results() {
    let (mut st, dir) = loaded("wfu-done");
    let op = || FileOp::UnstageGone("s.txt".into());
    st.session.diff.view = Some((DiffTarget::Staged("s.txt".into()), empty_diff().into()));
    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Ok(WorkFileOk::Unstaged) }));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Removed s.txt from the staging area."));
    assert!(st.session.diff.view.is_none(), "the staged diff of the unstaged file is closed");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");

    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Err("boom".into()) }));
    assert_eq!(last_log(&st), "Could not unstage s.txt: boom");

    for (result, want) in [
        (Ok(WorkFileOk::Unstaged), "Removed s.txt from the staging area."),
        (Err("in the way".to_owned()), "Could not unstage s.txt: in the way"),
    ] {
        let ok = result.is_ok();
        let msg = Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result });
        assert!(run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) }).is_empty());
        let text = if ok { st.toasts.last().map(|t| t.text.as_str()) } else { st.log.last().map(|l| l.text.as_str()) };
        assert_eq!(text, Some(want));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_delete_staged_rename() {
    let dir = scratch_repo("wfu-e2e-rename");
    std::fs::write(dir.join("b.txt"), "one\ntwo\nthree\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "b"]);
    git(&dir, &["mv", "b.txt", "c.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let renamed = FileEntry { renamed_from: Some("b.txt".into()), ..entry("c.txt", 'R') };
    assert!(st.snap.as_ref().unwrap().staged.contains(&renamed));

    drive(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "c.txt".into(), confirmed: false }));
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { title, .. }) if title == "Delete renamed file"));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(!dir.join("c.txt").exists());
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.iter().all(|f| f.path != "b.txt" && f.path != "c.txt"), "{:?}", snap.staged);
    assert!(snap.unstaged.contains(&entry("b.txt", 'D')), "{:?}", snap.unstaged);
    assert!(st.toasts.last().is_some_and(|t| t.ok), "{:?}", st.log.last().map(|l| &l.text));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_unstage_gone_new_file() {
    let dir = scratch_repo("wfu-e2e-gone");
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    git(&dir, &["add", "new.txt"]);
    std::fs::remove_file(dir.join("new.txt")).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.contains(&entry("new.txt", 'A')) && snap.unstaged.contains(&entry("new.txt", 'D')));

    drive(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false }));
    assert!(matches!(&st.modal, Some(Modal::Confirm { title, .. }) if title == "Unstage deleted file"));
    drive(&mut st, Msg::ConfirmAccepted);
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.iter().chain(&snap.unstaged).all(|f| f.path != "new.txt"), "{:?} {:?}", snap.staged, snap.unstaged);
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Removed new.txt from the staging area."));
    let _ = std::fs::remove_dir_all(&dir);
}
