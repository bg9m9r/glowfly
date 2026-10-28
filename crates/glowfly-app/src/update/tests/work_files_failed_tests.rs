//! Headless tests for file actions that failed after changing something: a delete whose file
//! went although the trash (or the index) reported an error, and a revert that changed the file
//! before failing. Both keep their undo entry, in the active tab's history or the parked one's,
//! and the log line says Undo puts the file back.

use super::tests::{drive, keys, scratch_repo, state};
use super::work_files_tests::{last_log, loaded, run};
use crate::effect::{DeleteMode, FileOp};
use crate::msg::{Inactive, JobMsg, Msg, RepoMsg, RevertWhat, StageMsg, WorkFileMsg};
use glowfly_core::undo::{Action, DeleteVia, Discarded, Saved};

fn delete(path: &str) -> FileOp {
    FileOp::Delete { path: path.into(), mode: DeleteMode::Trash }
}

fn undo_entry(path: &str) -> Box<Action> {
    let before = Saved { blob: git2::Oid::zero(), executable: false };
    Box::new(Action::DeleteFile { path: path.into(), before, via: DeleteVia::Trash, staged: None })
}

#[test]
fn a_delete_that_failed_after_the_file_went_records_its_undo_and_says_so() {
    let (mut st, dir) = loaded("wff-delete");
    let error = "a.txt was removed, but the Recycle Bin reported: no record".to_owned();
    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileFailed { op: delete("a.txt"), error, undo: undo_entry("a.txt") }));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::DeleteFile { path, .. }) if path == "a.txt"));
    assert_eq!(
        last_log(&st),
        format!(
            "Could not delete a.txt: a.txt was removed, but the Recycle Bin reported: no record. Undo ({}) puts it back.",
            keys("Ctrl+Z")
        )
    );
    assert!(st.log.last().is_some_and(|l| !l.ok));

    // For a tab that is no longer active: the log line and the undo entry go to that tab.
    let msg = WorkFileMsg::WorkFileFailed { op: delete("b.txt"), error: "boom".into(), undo: undo_entry("b.txt") };
    assert!(
        matches!(msg.on_inactive(), Some(Inactive::LogAndUndo { text, undo }) if text == "Could not delete b.txt: boom" && matches!(&undo, Action::DeleteFile { path, .. } if path == "b.txt"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_discard_whose_file_went_is_undone_from_the_kept_copy() {
    // The file is gone although the discard reported an error: the runner's message for that,
    // then Undo end to end.
    let dir = scratch_repo("wff-e2e");
    std::fs::write(dir.join("u.txt"), "work\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let blob = git2::Repository::open(&dir).unwrap().blob(b"work\n").unwrap();
    std::fs::remove_file(dir.join("u.txt")).unwrap();
    let undo = Box::new(Action::DeleteFile {
        path: "u.txt".into(),
        before: Saved { blob, executable: false },
        via: DeleteVia::Permanent,
        staged: None,
    });
    let op = FileOp::Discard { path: "u.txt".into(), mode: DeleteMode::Permanent };
    drive(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileFailed { op, error: "the index is locked".into(), undo }));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "work\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_revert_that_failed_after_changing_the_file_records_its_undo() {
    let (mut st, dir) = loaded("wff-revert");
    let undo = Action::Discard { path: "f.txt".into(), what: Discarded::RevertedHunk, before: None, after: None };
    let msg = |undo: Option<Action>| StageMsg::Reverted {
        path: "f.txt".into(),
        what: RevertWhat::Hunk,
        result: Err("the filter failed".into()),
        undo,
    };
    run(&mut st, Msg::Stage(msg(Some(undo.clone()))));
    assert_eq!(st.session.undo.undo.last(), Some(&undo));
    assert_eq!(
        last_log(&st),
        format!("Could not revert the hunk in f.txt: the filter failed. Undo ({}) puts back what it held before.", keys("Ctrl+Z"))
    );

    // A parked tab keeps it too; without an undo it is only logged.
    assert!(matches!(msg(Some(undo.clone())).on_inactive(), Some(Inactive::LogAndUndo { undo: a, .. }) if a == undo));
    assert!(matches!(msg(None).on_inactive(), Some(Inactive::Log(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_deleted_file_is_named_in_the_undo_menu() {
    let (mut st, dir) = loaded("wff-unstage-label");
    let staged = glowfly_core::repo::StagedNew { blob: git2::Oid::zero(), mode: 0o100644, source: None };
    let ok = crate::effect::WorkFileOk::Unstaged { undo: Some(Box::new(Action::UnstageGone { path: "n.txt".into(), staged })) };
    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: FileOp::UnstageGone("n.txt".into()), result: Ok(ok) }));
    assert_eq!(st.session.undo.undo_entry(st.settings.lang(), true).0, "Undo unstage of n.txt");
    let _ = std::fs::remove_dir_all(&dir);
}
