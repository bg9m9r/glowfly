//! Headless tests for deleting a staged rename's new path (unstage and delete, putting the
//! rename's source back in the index) and for unstaging a staged new file or rename whose
//! working copy is already gone: the confirmations, the results, and end to end against a
//! scratch repository.

use super::tests::{drive, git, keys, scratch_repo, state};
use super::work_files_tests::{confirm, empty_diff, entry, last_log, loaded, run, work_ops};
use glowfly_core::repo::{Deleted, DiffTarget, FileEntry};
use glowfly_model::effect::{DeleteMode, Effect, FileOp, WorkFileOk};
use glowfly_model::msg::JobMsg;
use glowfly_model::msg::Msg;
use glowfly_model::msg::RepoMsg;
use glowfly_model::msg::WorkFileMsg;
use glowfly_model::state::Modal;
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
    assert!(body.contains("brings back the file and the staged rename.") && body.ends_with("asks again before deleting one."), "{body}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn renamed(path: &str, from: &str) -> FileEntry {
    FileEntry { renamed_from: Some(from.into()), ..entry(path, 'R') }
}

#[test]
fn delete_staged_rename_names_the_old_path() {
    let (mut st, dir) = loaded("wfu-rename-from");
    st.settings.delete_to_trash = false;
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = Vec::new();
    snap.staged = vec![renamed("new.txt", "old.txt")];
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false })).is_empty());
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Delete renamed file", "Unstage and delete"));
    assert!(body.starts_with("new.txt is staged as a rename of old.txt."), "{body}");
    assert!(body.contains("old.txt stays deleted"), "{body}");
    assert!(!body.contains("the file it was renamed from"), "{body}");
    let op = FileOp::UnstageDelete { path: "new.txt".into(), mode: DeleteMode::Permanent };
    assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(o)) if *o == op));

    // With the working copy gone only the index changes: the same old path is named.
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("new.txt", 'D')];
    st.modal = None;
    run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "new.txt".into(), confirmed: false }));
    let (title, body, button, _) = confirm(&st);
    assert_eq!((title, button), ("Unstage deleted file", "Unstage"));
    assert!(body.starts_with("new.txt is staged as a rename of old.txt but is no longer in your working tree."), "{body}");
    assert!(body.contains("old.txt shows as deleted"), "{body}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_toast_names_the_old_path_of_the_rename_it_unstaged() {
    let (mut st, dir) = loaded("wfu-toast-from");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = Vec::new();
    snap.staged = vec![renamed("new.txt", "old.txt")];
    let op = FileOp::UnstageDelete { path: "new.txt".into(), mode: DeleteMode::Permanent };
    let done = |op: FileOp, ok: WorkFileOk| Msg::WorkFile(WorkFileMsg::WorkFileDone { op, result: Ok(ok) });
    // No undo entry (a file over 64 MB): the active snapshot still says what was unstaged.
    run(&mut st, done(op.clone(), WorkFileOk::deleted(Deleted::Permanently)));
    assert_eq!(
        st.newest_toast().map(|t| t.text.as_str()),
        Some("Deleted new.txt and removed it from the staging area. The rename from old.txt is unstaged, so old.txt shows as deleted.")
    );

    st.snap.as_mut().unwrap().staged = vec![renamed("s.txt", "old.txt")];
    run(&mut st, done(FileOp::UnstageGone("s.txt".into()), WorkFileOk::Unstaged { undo: None }));
    assert_eq!(
        st.newest_toast().map(|t| t.text.as_str()),
        Some("Removed s.txt from the staging area. The rename from old.txt is unstaged, so old.txt shows as deleted.")
    );

    // A staged new file has no old path.
    st.snap.as_mut().unwrap().staged = vec![entry("n.txt", 'A')];
    run(&mut st, done(FileOp::UnstageGone("n.txt".into()), WorkFileOk::Unstaged { undo: None }));
    assert_eq!(st.newest_toast().map(|t| t.text.as_str()), Some("Removed n.txt from the staging area."));

    // Another tab's result never reads this tab's snapshot.
    st.snap.as_mut().unwrap().staged = vec![renamed("new.txt", "old.txt")];
    let elsewhere = glowfly_core::paths::temp_dir().join("glowfly-elsewhere");
    let msg = done(op, WorkFileOk::deleted(Deleted::Permanently));
    assert!(run(&mut st, Msg::Scoped { workdir: elsewhere, msg: Box::new(msg) }).is_empty());
    assert_eq!(st.newest_toast().map(|t| t.text.as_str()), Some("Deleted new.txt and removed it from the staging area."));
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
    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Ok(WorkFileOk::Unstaged { undo: None }) }));
    assert_eq!(st.newest_toast().map(|t| t.text.as_str()), Some("Removed s.txt from the staging area."));
    assert!(st.session.diff.view.is_none(), "the staged diff of the unstaged file is closed");
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(glowfly_model::effect::LoadEffect::Status { .. }))), "{fx:?}");

    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Err(glowfly_model::testing::write_said("boom")) }));
    assert_eq!(last_log(&st), "Could not unstage s.txt: boom");

    for (result, want) in [
        (Ok(WorkFileOk::Unstaged { undo: None }), "Removed s.txt from the staging area."),
        (Err(glowfly_model::testing::write_said("in the way")), "Could not unstage s.txt: in the way"),
    ] {
        let ok = result.is_ok();
        let msg = Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result });
        assert!(run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) }).is_empty());
        let text = if ok { st.newest_toast().map(|t| t.text.as_str()) } else { st.log.last().map(|l| l.text.as_str()) };
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
    // The undo entry core kept names what it dissolved, before the Undo sentence.
    let text = st.toasts.last().map(|t| t.text.clone()).unwrap_or_default();
    assert!(text.contains("The rename from b.txt is unstaged, so b.txt shows as deleted."), "{text}");
    assert!(text.ends_with(&format!(" Undo ({}) puts it back.", keys("Ctrl+Z"))), "{text}");
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
    let want = format!("Removed new.txt from the staging area. Undo ({}) stages it again.", keys("Ctrl+Z"));
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == want), "{:?}", st.toasts.last().map(|t| &t.text));

    // Undo stages its kept blob again; the file stays gone from the working tree.
    drive(&mut st, Msg::Job(JobMsg::Undo));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.staged.contains(&entry("new.txt", 'A')) && snap.unstaged.contains(&entry("new.txt", 'D')), "{:?}", snap.staged);
    assert!(!dir.join("new.txt").exists());
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(st.snap.as_ref().unwrap().staged.iter().all(|f| f.path != "new.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}
