//! Headless tests for the edges of undoing conflict changes: a one-file toast offers no Undo
//! for a change of several files, an entry that can no longer be carried out goes alone (the
//! older ones stay), and a change that failed partway still records its undo.

use super::conflict_mark_undo_tests::{binary_conflict, opened, stages};
use super::conflict_tests::two_conflicts;
use super::tests::drive;
use crate::msg::{ConflictMsg, JobMsg, Msg, StageMsg};
use glowfly_core::repo::Side;
use glowfly_core::undo::{Action, WorkAt};

#[test]
fn a_one_file_toast_offers_no_undo_for_several_files() {
    let dir = two_conflicts("cms-two-files");
    let mut st = opened(&dir);
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(vec!["a.txt".into(), "b.txt".into()])));
    drive(&mut st, Msg::ConfirmAccepted);
    let t = st.toasts.last().expect("a toast");
    assert!(t.text.starts_with("Resolved a.txt"), "{}", t.text);
    assert!(t.action.is_none(), "Undo would put back b.txt too");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Conflicts { files, .. }) if files.len() == 2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_entry_that_cant_be_carried_out_goes_alone() {
    let dir = binary_conflict("cms-lost-blob");
    let mut st = opened(&dir);
    let older = Action::CreateTag { name: "v1".into(), oid: git2::Oid::zero() };
    st.session.undo.record(older.clone());
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("logo.bin".into())));
    drive(&mut st, Msg::Conflict(ConflictMsg::ResolveWithSide { path: "logo.bin".into(), side: Side::Theirs }));
    let Some(Action::Conflicts { files, .. }) = st.session.undo.undo.last() else { panic!("{:?}", st.session.undo.undo) };
    let WorkAt::File(ours) = files[0].before.work else { panic!() };
    let hex = ours.blob.to_string();
    std::fs::remove_file(dir.join(".git/objects").join(&hex[..2]).join(&hex[2..])).unwrap();
    let left = stages(&dir, "logo.bin");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let t = st.toasts.last().expect("a toast");
    assert!(!t.ok && t.text.contains("Glowfly no longer has"), "{}", t.text);
    assert_eq!(st.session.undo.undo, [older], "only the conflict change went");
    assert_eq!(stages(&dir, "logo.bin"), left, "nothing was written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_that_failed_partway_keeps_its_undo() {
    let dir = binary_conflict("cms-partway");
    let mut st = opened(&dir);
    let undo = Action::CreateTag { name: "stand-in".into(), oid: git2::Oid::zero() };
    let error = "b.txt failed. Undo puts back the files it changed before it stopped.".to_owned();
    drive(&mut st, Msg::Stage(StageMsg::LocalOpFailed { error: error.clone(), undo: undo.clone() }));
    assert_eq!(st.session.undo.undo.last(), Some(&undo));
    assert!(st.log.iter().any(|l| !l.ok && l.text == error));
    let _ = std::fs::remove_dir_all(&dir);
}
