//! Headless tests for staged renames (git mv) and New file in this folder: which renames are
//! staged, the remembered checkbox, the results, the folder prefill, and end to end against a
//! scratch repository.

use super::tests::{drive, scratch_repo, state};
use super::work_files_tests::{empty_diff, entry, loaded, new_form, rename_form, run, work_ops};
use crate::effect::{Effect, FileOp, WorkFileOk};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::WorkFileMsg;
use crate::state::Modal;
use crate::state::files::{PathForm, folder_prefill};
use glowfly_core::repo::{DiffTarget, FileEntry};

#[test]
fn rename_stages_only_tracked_files() {
    let (mut st, dir) = loaded("wf-stage-tracked");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![entry("a.txt", 'M'), entry("u.txt", '?'), entry("rc.txt", '?')];
    // rc.txt was removed from the index (rm --cached): staged 'D' plus untracked.
    snap.staged = vec![entry("s.txt", 'A'), entry("rc.txt", 'D')];
    let op = |from: &str, stage| FileOp::Rename { from: from.into(), to: "moved.txt".into(), stage };
    for (from, asked, staged) in
        [("a.txt", true, true), ("s.txt", true, true), ("u.txt", true, false), ("rc.txt", true, false), ("a.txt", false, false)]
    {
        run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFileDialog(from.into())));
        assert!(matches!(&st.modal, Some(Modal::RenameFile { old, .. }) if old == from), "{from}");
        let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFile { from: from.into(), to: "moved.txt".into(), stage: asked }));
        assert_eq!(work_ops(&fx), [op(from, staged)], "{from} asked {asked}");
        assert!(rename_form(&st).busy);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_result_toast_and_diff() {
    let (mut st, dir) = loaded("wf-stage-done");
    let op = || FileOp::Rename { from: "a.txt".into(), to: "b.txt".into(), stage: true };
    st.diff = Some((DiffTarget::Staged("a.txt".into()), empty_diff().into()));
    run(
        &mut st,
        Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Ok(WorkFileOk::Written { ignored: false, warnings: vec![] }) }),
    );
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Renamed a.txt to b.txt and staged the rename."));
    assert!(st.diff.is_none());

    // Never "ignored": the new path stays tracked.
    run(
        &mut st,
        Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Ok(WorkFileOk::Written { ignored: true, warnings: vec![] }) }),
    );
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Renamed a.txt to b.txt and staged the rename."));

    st.snap.as_mut().unwrap().unstaged = vec![entry("a.txt", 'M')];
    run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFileDialog("a.txt".into())));
    if let Some(Modal::RenameFile { form, .. }) = &mut st.modal {
        form.text = "b.txt".into();
    }
    run(&mut st, Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "b.txt".into(), stage: true }));
    assert!(rename_form(&st).busy);
    let err = "Unable to create 'x/.git/index.lock': File exists.";
    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: op(), result: Err(err.into()) }));
    let form = rename_form(&st);
    assert!(!form.busy);
    assert_eq!(form.error, Some(("b.txt".into(), err.into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_rename_stages_saves() {
    let mut st = state();
    assert!(st.settings.stage_renames, "on by default");
    let fx = run(&mut st, Msg::WorkFile(WorkFileMsg::SetRenameStages(false)));
    assert!(!st.settings.stage_renames);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if !s.stage_renames), "{fx:?}");
    assert!(run(&mut st, Msg::WorkFile(WorkFileMsg::SetRenameStages(false))).is_empty(), "no change, no save");
}

#[test]
fn new_file_here_prefill() {
    assert_eq!(folder_prefill("src/app.txt"), "src/");
    assert_eq!(folder_prefill("a/b/c.txt"), "a/b/");
    assert_eq!(folder_prefill("a.txt"), "");
    let (mut st, dir) = loaded("wf-new-here");
    run(&mut st, Msg::WorkFile(WorkFileMsg::NewFileDialog(folder_prefill("src/app.txt"))));
    assert_eq!(new_form(&st), &PathForm::new("src/"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_staged_rename() {
    let dir = scratch_repo("wf-e2e-staged");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "docs/b.txt".into(), stage: true }));
    assert!(dir.join("docs/b.txt").is_file());
    assert!(!dir.join("a.txt").exists());
    let snap = st.snap.as_ref().unwrap();
    let renamed = FileEntry { renamed_from: Some("a.txt".into()), ..entry("docs/b.txt", 'R') };
    assert!(snap.staged.contains(&renamed), "{:?}", snap.staged);
    // a.txt had an unstaged edit, which stays unstaged.
    assert!(snap.unstaged.contains(&entry("docs/b.txt", 'M')), "{:?}", snap.unstaged);
    assert!(st.toasts.last().is_some_and(|t| t.text.contains("staged the rename")), "{:?}", st.toasts.last().map(|t| &t.text));
    let _ = std::fs::remove_dir_all(&dir);
}
