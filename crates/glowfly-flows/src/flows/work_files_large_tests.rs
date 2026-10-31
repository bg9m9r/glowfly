//! Headless tests for a file Glowfly keeps no copy of for Undo (over 64 MB) when the trash
//! refuses it: the permanent delete it offers says it can't be undone and asks for the
//! acknowledgement, for Delete, Unstage and delete and Discard alike, and the runner tells
//! which files that is.

use super::tests::{keys, scratch_repo};
use super::work_files_tests::{ack, confirm, loaded, run, tick, work_ops};
use crate::effect::{DeleteMode, FileOp};
use crate::msg::{Msg, WorkFileMsg};
use crate::state::Modal;
use crate::state::files::TRASH;
use glowfly_core::repo::{TrashProblem, TrashRefusal};

fn ops() -> [FileOp; 3] {
    [
        FileOp::Delete { path: "big.bin".into(), mode: DeleteMode::Trash },
        FileOp::UnstageDelete { path: "big.bin".into(), mode: DeleteMode::Trash },
        FileOp::Discard { path: "big.bin".into(), mode: DeleteMode::Trash, seen: None },
    ]
}

fn refused(op: FileOp, problem: TrashProblem, too_large: bool) -> Msg {
    Msg::WorkFile(WorkFileMsg::TrashRefused { op, refusal: TrashRefusal::new(problem, "no bin here"), too_large })
}

#[test]
fn no_trash_for_a_file_without_a_copy_says_it_cant_be_undone_and_asks() {
    let (mut st, dir) = loaded("wfl-no-trash");
    for op in ops() {
        run(&mut st, refused(op.clone(), TrashProblem::NoTrash, true));
        let (title, body, _, then) = confirm(&st);
        assert_eq!(title, "Delete permanently?");
        assert!(
            body.starts_with(&format!(
                "big.bin couldn't be moved to the {TRASH}: no bin here.\n\nDelete it permanently instead? It's over 64 MB, so Glowfly kept no copy for Undo: this can't be undone."
            )),
            "{body}"
        );
        assert!(!body.contains("Undo ("), "no promise that Undo brings it back: {body}");
        assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(o)) if *o == op.clone().no_promise()), "no Undo promised");
        assert_eq!(ack(&st), Some("I understand this can't be undone"), "{op:?}");
        assert!(run(&mut st, Msg::ConfirmAccepted).is_empty(), "nothing runs before the tick");
        tick(&mut st);
        assert_eq!(work_ops(&run(&mut st, Msg::ConfirmAccepted)), [op.no_promise()]);
    }

    // A file with a copy keeps the Undo promise and needs no acknowledgement.
    run(&mut st, refused(ops()[2].clone(), TrashProblem::NoTrash, false));
    assert!(confirm(&st).1.contains(&format!("Undo ({}) can still bring it back", keys("Ctrl+Z"))));
    assert_eq!(ack(&st), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_home_trash_offer_for_a_file_without_a_copy_confirms_a_permanent_delete() {
    let (mut st, dir) = loaded("wfl-home");
    for op in ops() {
        run(&mut st, refused(op.clone(), TrashProblem::HomeTrashOnly, true));
        let Some(Modal::TrashOffer(o)) = &st.modal else { panic!("the home Trash offer") };
        assert!(
            o.body.ends_with("Or delete it permanently, which can't be undone: it's over 64 MB, so Glowfly kept no copy for Undo."),
            "{}",
            o.body
        );
        let (body, ack) = o.confirm.clone().expect("Delete permanently asks once more");
        assert_eq!(body, "big.bin is over 64 MB, so Glowfly kept no copy for Undo. Delete it permanently? This can't be undone.");
        assert_eq!(ack, "I understand this can't be undone");
    }
    run(&mut st, refused(ops()[0].clone(), TrashProblem::HomeTrashOnly, false));
    let Some(Modal::TrashOffer(o)) = &st.modal else { panic!("the home Trash offer") };
    assert_eq!(o.confirm, None, "a file with a copy is deleted straight away");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_runner_says_when_the_refused_file_is_too_large_to_keep() {
    // A stand-in trash that refuses everything: the file stays, and its size decides.
    let dir = scratch_repo("wfl-runner");
    std::fs::write(dir.join("small.txt"), "small\n").unwrap();
    let big = std::fs::File::create(dir.join("big.bin")).unwrap();
    big.set_len(glowfly_core::undo::MAX_KEPT_BYTES + 1).unwrap();
    drop(big);
    for (path, want) in [("small.txt", false), ("big.bin", true)] {
        let op = FileOp::Delete { path: path.into(), mode: DeleteMode::Trash };
        let msg = crate::runner::testing::refused_for_test(&dir, op);
        assert!(matches!(msg, Msg::WorkFile(WorkFileMsg::TrashRefused { too_large, .. }) if too_large == want), "{path}");
        assert!(dir.join(path).exists());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file that grows past 64 MB while a confirmation promising Undo is open: the runner leaves
/// it, and Glowfly asks again with the acknowledgement before deleting it for good.
#[test]
fn a_file_that_grew_too_large_after_undo_was_promised_is_asked_about_again() {
    let (mut st, dir) = loaded("wfl-grew");
    let big = std::fs::File::create(dir.join("big.bin")).unwrap();
    big.set_len(glowfly_core::undo::MAX_KEPT_BYTES + 1).unwrap();
    drop(big);
    for op in [
        FileOp::Delete { path: "big.bin".into(), mode: DeleteMode::Permanent },
        FileOp::Discard { path: "big.bin".into(), mode: DeleteMode::PermanentNoTrash, seen: None },
    ] {
        let msg = crate::runner::testing::work_file_for_test(&dir, op.clone());
        assert!(matches!(&msg, Msg::WorkFile(WorkFileMsg::NoCopyKept { .. })), "{op:?}");
        assert!(dir.join("big.bin").exists(), "nothing deleted");
        run(&mut st, msg);
        let (title, body, _, then) = confirm(&st);
        assert_eq!(title, "Delete permanently?");
        assert_eq!(
            body,
            "big.bin is over 64 MB, so Glowfly can't keep a copy for Undo, and nothing was deleted. Delete it permanently anyway? This can't be undone."
        );
        assert!(matches!(then, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(o)) if *o == op.clone().no_promise()));
        assert_eq!(ack(&st), Some("I understand this can't be undone"));
        assert!(run(&mut st, Msg::ConfirmAccepted).is_empty(), "nothing runs before the tick");
        tick(&mut st);
        assert_eq!(work_ops(&run(&mut st, Msg::ConfirmAccepted)), [op.no_promise()]);
    }
    // Acknowledged, it goes with no copy.
    let msg =
        crate::runner::testing::work_file_for_test(&dir, FileOp::Delete { path: "big.bin".into(), mode: DeleteMode::PermanentNoPromise });
    assert!(matches!(msg, Msg::WorkFile(WorkFileMsg::WorkFileDone { result: Ok(_), .. })));
    assert!(!dir.join("big.bin").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_copy_kept_for_an_inactive_tab_is_logged() {
    let op = FileOp::Delete { path: "big.bin".into(), mode: DeleteMode::Permanent };
    let msg = WorkFileMsg::NoCopyKept { op, reason: "big.bin is too large for Glowfly to keep a copy for Undo".into() };
    let Some(crate::msg::Inactive::Log(text)) = msg.on_inactive() else { panic!("a log line") };
    assert_eq!(text, "Did not delete big.bin: big.bin is too large for Glowfly to keep a copy for Undo.");
}
