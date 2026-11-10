//! Headless tests for deleting several picked WIP files, end to end through the runner with the
//! test build's stand-in trash: the picks (with a gap) go after one confirmation, as one undo
//! entry that Undo puts back byte for byte and Redo deletes again; a staged-new pick leaves the
//! index and comes back staged; a trash that refuses every file changes nothing; and a delete
//! that stops partway keeps Undo for the files before it.

use super::tests::{drive, git};
use super::update;
use super::wip_picks_tests::{c_text, in_bin, listed, pick, picked_a_and_c, read};
use glowfly_core::repo::DiffTarget;
use glowfly_core::undo::{Action, BatchOp};
use glowfly_model::effect::{Effect, FilesEffect};
use glowfly_model::msg::{JobMsg, Msg, RepoMsg, WorkFileMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::wip_lists::picked_paths;
use std::time::Instant;

/// Sends what the selection menu's Delete N files... sends for the picks of that list.
fn delete_picks(st: &mut AppState, staged: bool) {
    let msg = Msg::WorkFile(WorkFileMsg::DeletePaths(picked_paths(st, staged)));
    drive(st, msg);
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

#[test]
fn delete_picks_with_gaps_then_undo_and_redo() {
    let (mut st, dir) = picked_a_and_c("picks-del-flow");
    st.settings.delete_to_trash = true;
    let bin = in_bin(&c_text(&dir));
    delete_picks(&mut st, false);
    assert!(st.modal.is_some(), "one confirmation");
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(read(&dir, "a.txt"), None);
    assert_eq!(read(&dir, "c.txt"), None);
    assert_eq!(in_bin(&c_text(&dir)), bin + 1, "in the stand-in trash");
    assert_eq!(read(&dir, "b.txt").as_deref(), Some("b edited\n"), "not picked, untouched");
    assert_eq!(st.session.undo.undo.len(), 1, "one undo entry: {:?}", st.session.undo);
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Batch { op: BatchOp::Delete, parts }) if parts.len() == 2));
    assert!(st.cues.wip_picks.is_none());

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\ntwo\n"));
    assert_eq!(read(&dir, "c.txt"), Some(c_text(&dir)));
    assert_eq!(st.session.undo.redo.len(), 1, "{:?}", st.session.undo);

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(read(&dir, "a.txt"), None, "deleted again");
    assert_eq!(read(&dir, "c.txt"), None, "deleted again");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Batch { op: BatchOp::Delete, .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_new_pick_leaves_the_index_and_undo_stages_it_again() {
    let (mut st, dir) = picked_a_and_c("picks-del-staged");
    st.settings.delete_to_trash = true;
    std::fs::write(dir.join("n.txt"), format!("new {}\n", dir.display())).unwrap();
    git(&dir, &["add", "a.txt", "n.txt"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("a.txt".into()))));
    drive(&mut st, pick(true, "n.txt"));
    assert_eq!(picked_paths(&st, true), ["a.txt", "n.txt"]);
    delete_picks(&mut st, true);
    drive(&mut st, Msg::ConfirmAccepted);
    assert_eq!(read(&dir, "a.txt"), None);
    assert_eq!(read(&dir, "n.txt"), None);
    assert_eq!(listed(&st, true), ["a.txt"], "n.txt left the index; a.txt keeps its staged version");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\ntwo\nthree\n"), "the working copy with its edits");
    assert_eq!(read(&dir, "n.txt"), Some(format!("new {}\n", dir.display())));
    assert_eq!(listed(&st, true), ["a.txt", "n.txt"], "n.txt staged again");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Accepts the open confirmation and returns the multi-file delete it asks the runner for.
fn confirmed_run(st: &mut AppState) -> (std::path::PathBuf, Vec<glowfly_model::effect::DeleteEach>, glowfly_model::effect::DeleteMode) {
    let fx = update(st, Msg::ConfirmAccepted, Instant::now());
    fx.into_iter()
        .find_map(|e| match e {
            Effect::Files(FilesEffect::DeletePaths { workdir, files, mode }) => Some((workdir, files, mode)),
            _ => None,
        })
        .expect("the multi-file delete")
}

#[test]
fn a_trash_that_refuses_changes_nothing() {
    let (mut st, dir) = picked_a_and_c("picks-del-refused");
    st.settings.delete_to_trash = true;
    delete_picks(&mut st, false);
    let (workdir, files, mode) = confirmed_run(&mut st);
    drive(&mut st, glowfly_runner::testing::delete_paths_refused_for_test(&workdir, files, mode));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\ntwo\n"));
    assert_eq!(read(&dir, "c.txt"), Some(c_text(&dir)));
    assert!(last_log(&st).starts_with("Did not delete the selected files: a.txt couldn't be deleted: "), "{}", last_log(&st));
    assert!(st.session.undo.undo.is_empty(), "{:?}", st.session.undo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_delete_that_stops_partway_keeps_undo_for_the_files_before() {
    let (mut st, dir) = picked_a_and_c("picks-del-stopped");
    st.settings.delete_to_trash = true;
    delete_picks(&mut st, false);
    let (_, files, mode) = confirmed_run(&mut st);
    // c.txt goes elsewhere before the run reaches it: a.txt is deleted, then the run stops.
    std::fs::remove_file(dir.join("c.txt")).unwrap();
    drive(&mut st, Msg::WorkFile(WorkFileMsg::DeletePathsConfirmed { files, mode }));
    assert_eq!(read(&dir, "a.txt"), None);
    assert!(last_log(&st).starts_with("Deleted 1 file, then stopped at c.txt: "), "{}", last_log(&st));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::DeleteFile { path, .. }) if path == "a.txt"), "{:?}", st.session.undo);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\ntwo\n"));
    let _ = std::fs::remove_dir_all(&dir);
}
