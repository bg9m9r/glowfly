//! Headless tests for deleting to the Recycle Bin / Trash: the confirmations' wording in trash
//! and permanent mode, the mode fixed when a confirmation opens, the permanent fallback offered
//! when the trash refuses a file, the toasts, and end to end with the test build's stand-in
//! trash (never the real one).

use super::tests::{drive, keys, scratch_repo, state};
use super::work_files_tests::{confirm, entry, last_log, loaded, run, work_ops};
use crate::effect::{DeleteMode, Effect, FileOp, WorkFileOk};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::WorkFileMsg;
use crate::state::Modal;
use crate::state::files::TRASH;
use glowfly_core::repo::{Deleted, TrashProblem, TrashRefusal};
use std::path::PathBuf;

fn delete(path: &str, mode: DeleteMode) -> FileOp {
    FileOp::Delete { path: path.into(), mode }
}

fn unstage_delete(path: &str, mode: DeleteMode) -> FileOp {
    FileOp::UnstageDelete { path: path.into(), mode }
}

/// Opens the confirmation for `path` and returns its body, button and the op it runs.
fn ask(st: &mut crate::state::AppState, path: &str) -> (String, String, FileOp) {
    assert!(run(st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: path.into(), confirmed: false })).is_empty());
    let (_, body, button, then) = confirm(st);
    let Msg::WorkFile(WorkFileMsg::DeleteConfirmed(op)) = then else { panic!("expected DeleteConfirmed for {path}") };
    (body.to_owned(), button.to_owned(), op.clone())
}

#[test]
fn confirm_says_trash_by_default() {
    let (mut st, dir) = loaded("wft-confirm");
    assert!(st.settings.delete_to_trash, "on by default");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("u.txt", '?'), entry("m.txt", 'M')];
    snap.staged = vec![entry("s.txt", 'M')];
    for (path, want) in [
        ("u.txt", format!("u.txt isn't tracked by git. It goes to the {TRASH}, so you can restore it from there.")),
        (
            "m.txt",
            format!(
                "m.txt goes to the {TRASH} with your uncommitted changes. Discarding the deletion afterwards brings back its staged or committed version instead."
            ),
        ),
        ("s.txt", format!("s.txt goes to the {TRASH}. Discarding the deletion brings it back as it is now.")),
        ("c.txt", format!("c.txt goes to the {TRASH}. Discarding the deletion brings it back from the last commit.")),
    ] {
        let (body, button, op) = ask(&mut st, path);
        assert_eq!(confirm(&st).0, "Delete file");
        assert_eq!(body, want);
        assert_eq!(button, format!("Move to {TRASH}"));
        assert_eq!(op, delete(path, DeleteMode::Trash));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn permanent_preference_keeps_old_texts() {
    let (mut st, dir) = loaded("wft-permanent");
    st.settings.delete_to_trash = false;
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("u.txt", '?')];
    snap.staged = Vec::new();
    let (body, button, op) = ask(&mut st, "u.txt");
    assert_eq!(
        body,
        format!(
            "u.txt isn't tracked by git. Glowfly keeps a copy so Undo ({}) can bring it back while this tab is open (files up to 64 MB).",
            keys("Ctrl+Z")
        )
    );
    assert_eq!(button, "Delete");
    assert_eq!(op, delete("u.txt", DeleteMode::Permanent));
    let (body, ..) = ask(&mut st, "c.txt");
    assert_eq!(body, "c.txt is deleted from your working tree. Discarding the deletion brings it back from the last commit.");
    assert!(!body.contains(TRASH));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_confirm_trash() {
    let (mut st, dir) = loaded("wft-unstage");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = Vec::new();
    snap.staged = vec![entry("n.txt", 'A'), entry("r.txt", 'R')];

    let (body, button, op) = ask(&mut st, "n.txt");
    assert_eq!(
        body,
        format!(
            "n.txt is a new file in the staging area. It goes to the {TRASH} and leaves the staging area; Undo ({}) stages it again.",
            keys("Ctrl+Z")
        )
    );
    assert_eq!(button, "Unstage and delete");
    assert_eq!(confirm(&st).0, "Delete new file");
    assert_eq!(op, unstage_delete("n.txt", DeleteMode::Trash));

    let (body, _, op) = ask(&mut st, "r.txt");
    assert!(body.starts_with(&format!("r.txt is staged as a rename. It goes to the {TRASH} and the rename is unstaged:")), "{body}");
    assert!(body.contains("the file it was renamed from stays deleted"), "{body}");
    assert!(!body.contains("can't be brought back"), "{body}");
    assert!(body.ends_with(&format!(" Undo ({}) brings back the file and the staged rename.", keys("Ctrl+Z"))), "{body}");
    assert_eq!(confirm(&st).0, "Delete renamed file");
    assert_eq!(op, unstage_delete("r.txt", DeleteMode::Trash));

    st.snap.as_mut().unwrap().unstaged = vec![entry("n.txt", 'M')];
    let (body, ..) = ask(&mut st, "n.txt");
    assert!(
        body.ends_with(&format!(
            " The copy in the {TRASH} has your unstaged edits; Undo ({}) brings back the staged version too.",
            keys("Ctrl+Z")
        )),
        "{body}"
    );

    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("n.txt", 'D')];
    let (body, button, op) = ask(&mut st, "n.txt");
    assert_eq!(confirm(&st).0, "Unstage deleted file");
    assert_eq!(button, "Unstage");
    assert!(!body.contains(TRASH), "{body}");
    assert_eq!(op, FileOp::UnstageGone("n.txt".into()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mode_is_fixed_at_ask_time() {
    let (mut st, dir) = loaded("wft-fixed");
    st.snap.as_mut().unwrap().unstaged = vec![entry("u.txt", '?')];
    ask(&mut st, "u.txt");
    st.settings.delete_to_trash = false;
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [delete("u.txt", DeleteMode::Trash)]);

    ask(&mut st, "u.txt");
    st.settings.delete_to_trash = true;
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [delete("u.txt", DeleteMode::Permanent)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_confirmed_only_runs_delete_ops() {
    let (mut st, dir) = loaded("wft-only-deletes");
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::Create("x.txt".into())))).is_empty());
    let rename = FileOp::Rename { from: "a.txt".into(), to: "b.txt".into(), stage: false };
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(rename))).is_empty());
    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(delete("a.txt", DeleteMode::Trash))));
    assert_eq!(work_ops(&fx), [delete("a.txt", DeleteMode::Trash)]);
    assert_eq!(fx[0].scope(), Some(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trash_refused_offers_permanent() {
    let (mut st, dir) = loaded("wft-refused");
    let fx = run(
        &mut st,
        Msg::WorkFile(WorkFileMsg::TrashRefused {
            op: delete("a.txt", DeleteMode::Trash),
            refusal: TrashRefusal::new(TrashProblem::NoTrash, "this drive has no Recycle Bin"),
        }),
    );
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. })), "{fx:?}");
    let (title, body, button, then) = confirm(&st);
    assert_eq!((title, button), ("Delete permanently?", "Delete permanently"));
    assert_eq!(
        body,
        format!(
            "a.txt couldn't be moved to the {TRASH}: this drive has no Recycle Bin.\n\nDelete it permanently instead? It won't be in the {TRASH}, but Undo ({}) can still bring it back while this tab is open.",
            keys("Ctrl+Z")
        )
    );
    assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(op)) if *op == delete("a.txt", DeleteMode::PermanentNoTrash)));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(work_ops(&fx), [delete("a.txt", DeleteMode::PermanentNoTrash)]);

    run(
        &mut st,
        Msg::WorkFile(WorkFileMsg::TrashRefused {
            op: unstage_delete("n.txt", DeleteMode::Trash),
            refusal: TrashRefusal::new(TrashProblem::NoTrash, "no bin"),
        }),
    );
    let (_, body, _, then) = confirm(&st);
    assert!(body.ends_with("while this tab is open. It also leaves the staging area."), "{body}");
    assert!(
        matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(op)) if *op == unstage_delete("n.txt", DeleteMode::PermanentNoTrash))
    );

    // Cancelling changes nothing.
    assert!(run(&mut st, Msg::CloseModal).is_empty());
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_toasts() {
    let (mut st, dir) = loaded("wft-toasts");
    let cases = [
        (delete("a.txt", DeleteMode::Trash), Deleted::Trashed, format!("Moved a.txt to the {TRASH}.")),
        (
            unstage_delete("a.txt", DeleteMode::Trash),
            Deleted::Trashed,
            format!("Moved a.txt to the {TRASH} and removed it from the staging area."),
        ),
        (
            delete("a.txt", DeleteMode::Trash),
            Deleted::LinkRemoved,
            format!("Deleted the symbolic link a.txt. Links don't go to the {TRASH}; what it points to is untouched."),
        ),
        (
            unstage_delete("a.txt", DeleteMode::Trash),
            Deleted::LinkRemoved,
            format!(
                "Deleted the symbolic link a.txt and removed it from the staging area. Links don't go to the {TRASH}; what it points to is untouched."
            ),
        ),
        (delete("a.txt", DeleteMode::Permanent), Deleted::Permanently, "Deleted a.txt.".to_owned()),
        (
            unstage_delete("a.txt", DeleteMode::Permanent),
            Deleted::Permanently,
            "Deleted a.txt and removed it from the staging area.".to_owned(),
        ),
        (
            delete("a.txt", DeleteMode::PermanentNoTrash),
            Deleted::Permanently,
            format!("Deleted a.txt permanently; the {TRASH} couldn't take it."),
        ),
        (
            unstage_delete("a.txt", DeleteMode::PermanentNoTrash),
            Deleted::Permanently,
            format!("Deleted a.txt permanently and removed it from the staging area; the {TRASH} couldn't take it."),
        ),
    ];
    for (op, how, want) in cases {
        run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op.clone(), result: Ok(WorkFileOk::deleted(how)) }));
        assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some(want.as_str()), "{op:?} {how:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trash_refused_on_inactive_tab_logs() {
    let (mut st, dir) = loaded("wft-scoped");
    let msg = Msg::WorkFile(WorkFileMsg::TrashRefused {
        op: delete("x.txt", DeleteMode::Trash),
        refusal: TrashRefusal::new(TrashProblem::NoTrash, "no bin"),
    });
    let fx = run(&mut st, Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(msg) });
    assert!(fx.is_empty());
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "Did not delete x.txt: no bin.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trash_refused_for_other_reasons_offers_retry_never_a_permanent_delete() {
    let (mut st, dir) = loaded("wft-retry");
    let cases = [
        (
            TrashProblem::InUse,
            "windows error: The process cannot access the file because it is being used by another process. (0x80070020)",
            "File in use".to_owned(),
            format!("a.txt is open in another program, so it couldn't be moved to the {TRASH}.\n\nClose it there, then try again."),
        ),
        (
            TrashProblem::AccessDenied,
            "Permission denied (os error 13)",
            format!("Can't move to {TRASH}"),
            format!(
                "Glowfly isn't allowed to move a.txt to the {TRASH}: Permission denied (os error 13).\n\nCheck that the file isn't locked and that you may change its folder, then try again."
            ),
        ),
        (
            TrashProblem::Other,
            "Some operations were aborted",
            format!("Can't move to {TRASH}"),
            format!(
                "a.txt couldn't be moved to the {TRASH}: Some operations were aborted.\n\nTry again. To delete files for good instead, turn off \"Move deleted files to the {TRASH}\" in Preferences."
            ),
        ),
    ];
    for op in [
        delete("a.txt", DeleteMode::Trash),
        unstage_delete("a.txt", DeleteMode::Trash),
        FileOp::Discard { path: "a.txt".into(), mode: DeleteMode::Trash },
    ] {
        for (problem, detail, want_title, want_body) in &cases {
            let refusal = TrashRefusal::new(*problem, *detail);
            run(&mut st, Msg::WorkFile(WorkFileMsg::TrashRefused { op: op.clone(), refusal }));
            let (title, body, button, then) = confirm(&st);
            assert_eq!((title, body, button), (want_title.as_str(), want_body.as_str(), "Retry"), "{problem:?} {op:?}");
            assert!(!body.contains("permanently"), "{body}");
            // Retry runs the same delete, to the trash again.
            assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(o)) if *o == op));
            let fx = run(&mut st, Msg::ConfirmAccepted);
            assert_eq!(work_ops(&fx), std::slice::from_ref(&op));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_trash() {
    let dir = scratch_repo("wft-e2e");
    let contents = format!("trash me {}\n", dir.display());
    std::fs::write(dir.join("a.txt"), &contents).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "a.txt".into(), confirmed: false }));
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { button, .. }) if *button == format!("Move to {TRASH}")));
    drive(&mut st, Msg::ConfirmAccepted);

    assert!(!dir.join("a.txt").exists());
    let bin = crate::runner::files::test_bin();
    let landed = std::fs::read_dir(&bin)
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().ends_with("-a.txt") && std::fs::read_to_string(e.path()).is_ok_and(|t| t == contents));
    assert!(landed, "a.txt is in the stand-in bin {}", bin.display());
    let want = format!("Moved a.txt to the {TRASH}. Undo ({}) puts it back.", keys("Ctrl+Z"));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some(want.as_str()));
    assert!(st.snap.as_ref().unwrap().unstaged.contains(&entry("a.txt", 'D')));
    let _ = std::fs::remove_dir_all(&dir);
}
