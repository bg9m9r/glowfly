//! Headless tests for deleting several picked WIP files (`work_files_delete_many.rs`): the
//! confirmation names the count and where the files go, its button sends one multi-file delete
//! in on-screen order with each file's unstage and promise flags; a hazard or a file gone from
//! the working tree blocks it; one file left takes the single flow; and the result records one
//! undo entry, toasts, or logs where it stopped (also in a tab that isn't active).

use super::tests::{scratch_repo, state, write_said};
use super::wip_picks_tests::{lists, pick, send};
use crate::effect::{DeleteEach, DeleteMode, Effect, FilesEffect};
use crate::msg::{FailedWrite, Inactive, Msg, RepoMsg, TabsMsg, WorkFileMsg};
use crate::state::files::TRASH;
use crate::state::wip_lists::PickHow;
use crate::state::{AppState, Modal, Tab};
use glowfly_core::repo::{DiffTarget, Hazard, LoadOptions, load};
use glowfly_core::undo::{Action, BatchOp, DeleteVia, Saved};

fn paths(ps: &[&str]) -> Vec<String> {
    ps.iter().map(|p| (*p).to_owned()).collect()
}

/// a.txt and c.txt (untracked) picked in Unstaged Files, b.txt between them left out.
fn a_and_c(name: &str) -> AppState {
    let mut st = lists(name);
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    st
}

fn each(path: &str, unstage: bool, promise: bool) -> DeleteEach {
    DeleteEach { path: path.into(), unstage, promise }
}

fn gone(path: &str) -> Action {
    let before = Saved::file(git2::Oid::from_bytes(&[3; 20]).unwrap(), false);
    Action::DeleteFile { path: path.into(), before, via: DeleteVia::Trash, staged: None }
}

fn last_log(st: &AppState) -> &str {
    st.log.last().map(|l| l.text.as_str()).unwrap_or_default()
}

#[test]
fn delete_paths_asks_once_naming_the_count_and_the_trash() {
    let mut st = a_and_c("picks-del-trash");
    st.settings.delete_to_trash = true;
    let fx = send(&mut st, Msg::WorkFile(WorkFileMsg::DeletePaths(paths(&["a.txt", "c.txt"]))));
    assert!(fx.is_empty());
    let Some(Modal::ConfirmDanger { title, body, button, then, .. }) = &st.modal else { panic!("a danger confirmation") };
    assert_eq!(title, "Delete 2 files");
    assert!(body.starts_with(&format!("The 2 selected files go to the {TRASH};")), "{body}");
    assert!(!body.contains("staging area"), "{body}");
    assert_eq!(*button, format!("Move 2 files to the {TRASH}"));
    let want = vec![each("a.txt", false, true), each("c.txt", false, true)];
    assert!(
        matches!(&**then, Msg::WorkFile(WorkFileMsg::DeletePathsConfirmed { files, mode: DeleteMode::Trash }) if *files == want),
        "in on-screen order, with the settings' mode"
    );
    assert_eq!(crate::state::wip_lists::picked_paths(&st, false), ["a.txt", "c.txt"], "the picks hold until it runs");
}

#[test]
fn delete_paths_for_good_says_copies_are_kept_and_names_staged_new_files() {
    let mut st = lists("picks-del-permanent");
    st.settings.delete_to_trash = false;
    send(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("x.txt".into()))));
    pick(&mut st, true, "z.txt", PickHow::Toggle);
    send(&mut st, Msg::WorkFile(WorkFileMsg::DeletePaths(paths(&["x.txt", "z.txt"]))));
    let Some(Modal::ConfirmDanger { title, body, button, then, .. }) = &st.modal else { panic!("a danger confirmation") };
    assert_eq!(title, "Delete 2 files");
    assert!(body.starts_with("The 2 selected files are deleted from your working tree. Glowfly keeps copies"), "{body}");
    assert!(body.ends_with("1 of them is new in the staging area and leaves it too."), "{body}");
    assert_eq!(button, "Delete 2 files");
    // x.txt is staged only, so git holds its content; z.txt is staged as new.
    let want = vec![each("x.txt", false, false), each("z.txt", true, true)];
    assert!(
        matches!(&**then, Msg::WorkFile(WorkFileMsg::DeletePathsConfirmed { files, mode: DeleteMode::Permanent }) if *files == want),
        "x.txt is staged only, z.txt staged as new"
    );
}

#[test]
fn confirmed_delete_sends_one_effect_and_clears_picks() {
    let mut st = a_and_c("picks-del-confirmed");
    let files = vec![each("a.txt", false, true), each("c.txt", false, true)];
    let fx = send(&mut st, Msg::WorkFile(WorkFileMsg::DeletePathsConfirmed { files: files.clone(), mode: DeleteMode::Trash }));
    let runs: Vec<_> = fx.iter().filter(|e| matches!(e, Effect::Files(FilesEffect::DeletePaths { .. }))).collect();
    assert_eq!(runs.len(), 1, "{fx:?}");
    assert!(matches!(runs[0], Effect::Files(FilesEffect::DeletePaths { files: f, mode: DeleteMode::Trash, .. }) if *f == files));
    assert!(st.cues.wip_picks.is_none());
}

#[test]
fn a_pick_with_a_hazard_blocks_the_delete() {
    let mut st = a_and_c("picks-del-hazard");
    let c = st.snap.as_mut().unwrap().unstaged.iter_mut().find(|f| f.path == "c.txt").unwrap();
    c.hazard = Some(Hazard::Twins(vec!["C.txt".into()]));
    send(&mut st, Msg::WorkFile(WorkFileMsg::DeletePaths(paths(&["a.txt", "c.txt"]))));
    assert!(st.modal.is_none());
    assert!(last_log(&st).starts_with("Can't delete the selected files because of c.txt."), "{}", last_log(&st));
}

#[test]
fn a_pick_gone_from_the_working_tree_blocks_the_delete() {
    let mut st = a_and_c("picks-del-gone");
    let a = st.snap.as_mut().unwrap().unstaged.iter_mut().find(|f| f.path == "a.txt").unwrap();
    a.status = 'D';
    send(&mut st, Msg::WorkFile(WorkFileMsg::DeletePaths(paths(&["a.txt", "c.txt"]))));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "a.txt isn't in your working tree");
}

#[test]
fn delete_with_one_pick_left_uses_the_single_flow() {
    let mut st = a_and_c("picks-del-one");
    send(&mut st, Msg::WorkFile(WorkFileMsg::DeletePaths(paths(&["a.txt", "gone.txt"]))));
    let Some(Modal::ConfirmDanger { title, .. }) = &st.modal else { panic!("the single confirmation") };
    assert_eq!(title, "Delete file");
}

#[test]
fn paths_deleted_records_one_undo_entry_and_toasts() {
    let mut st = a_and_c("picks-del-done");
    let undo = glowfly_core::undo::batch(BatchOp::Delete, vec![gone("a.txt"), gone("c.txt")]);
    let msg = WorkFileMsg::PathsDeleted { mode: DeleteMode::Trash, done: 2, no_undo: 0, failed: None, undo };
    send(&mut st, Msg::WorkFile(msg));
    assert_eq!(st.session.undo.undo.len(), 1);
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Batch { op: BatchOp::Delete, parts }) if parts.len() == 2));
    let toast = st.toasts.last().map(|t| t.text.as_str()).unwrap_or_default();
    assert!(toast.starts_with(&format!("Moved 2 files to the {TRASH}. Undo")), "{toast}");

    let msg = WorkFileMsg::PathsDeleted { mode: DeleteMode::Permanent, done: 3, no_undo: 1, failed: None, undo: Some(gone("a.txt")) };
    send(&mut st, Msg::WorkFile(msg));
    let toast = st.toasts.last().map(|t| t.text.as_str()).unwrap_or_default();
    assert!(toast.starts_with("Deleted 3 files. Undo"), "{toast}");
    assert!(toast.ends_with("Undo can't put back 1 of them (folder links, or files over 64 MB that Git holds)."), "{toast}");
}

#[test]
fn a_stopped_delete_logs_the_file_and_keeps_the_undo() {
    let mut st = a_and_c("picks-del-stopped");
    let failed = Some(("c.txt".to_owned(), write_said("in use")));
    let msg = WorkFileMsg::PathsDeleted { mode: DeleteMode::Trash, done: 1, no_undo: 0, failed, undo: Some(gone("a.txt")) };
    send(&mut st, Msg::WorkFile(msg));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::DeleteFile { path, .. }) if path == "a.txt"));
    assert!(last_log(&st).starts_with("Deleted 1 file, then stopped at c.txt: "), "{}", last_log(&st));
    assert!(last_log(&st).contains("puts back the one deleted"), "{}", last_log(&st));

    let failed = Some(("a.txt".to_owned(), write_said("in use")));
    let msg = WorkFileMsg::PathsDeleted { mode: DeleteMode::Trash, done: 0, no_undo: 0, failed, undo: None };
    send(&mut st, Msg::WorkFile(msg));
    assert!(last_log(&st).starts_with("Did not delete the selected files: a.txt couldn't be deleted: "), "{}", last_log(&st));
}

#[test]
fn paths_deleted_for_an_inactive_tab_goes_to_its_history() {
    let undo = glowfly_core::undo::batch(BatchOp::Delete, vec![gone("a.txt"), gone("b.txt")]);
    let ok = WorkFileMsg::PathsDeleted { mode: DeleteMode::Trash, done: 2, no_undo: 0, failed: None, undo: undo.clone() };
    assert!(matches!(ok.on_inactive(), Some(Inactive::Undo(Action::Batch { .. }))));
    let failed = Some(("c.txt".to_owned(), write_said("in use")));
    let stopped = WorkFileMsg::PathsDeleted { mode: DeleteMode::Trash, done: 2, no_undo: 0, failed, undo };
    let Some(Inactive::WriteFailed { at, undo, .. }) = stopped.on_inactive() else { panic!("a failed write") };
    assert_eq!(at, FailedWrite::DeletePaths { path: "c.txt".into(), done: 2 });
    assert!(undo.is_some());
    let none = WorkFileMsg::PathsDeleted { mode: DeleteMode::Trash, done: 0, no_undo: 0, failed: None, undo: None };
    assert!(none.on_inactive().is_none());
}

#[test]
fn a_stopped_delete_in_a_parked_tab_logs_there_without_a_key() {
    let dir = scratch_repo("picks-del-parked");
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    st.snap = Some(load(&dir, &LoadOptions::all(10)).unwrap());
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(st.active_tab, 1);
    let failed = Some(("c.txt".to_owned(), write_said("in use")));
    let msg = WorkFileMsg::PathsDeleted { mode: DeleteMode::Trash, done: 1, no_undo: 0, failed, undo: Some(gone("a.txt")) };
    send(&mut st, Msg::Scoped { workdir: dir.clone(), msg: Box::new(Msg::WorkFile(msg)) });
    let line = st.tabs[0].log.last().map(|l| l.text.clone()).unwrap_or_default();
    assert!(line.starts_with("Deleted 1 file, then stopped at c.txt: "), "{line}");
    assert!(line.contains(". Undo puts back the one deleted."), "no key hint: {line}");
    let _ = std::fs::remove_dir_all(&dir);
}
