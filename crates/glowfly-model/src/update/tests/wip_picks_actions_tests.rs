//! Headless tests for the actions on WIP files picked with gaps: Space, S and U stage or
//! unstage every pick in one operation; Delete asks once, naming the count, and its confirmation
//! sends one multi-file discard; a pick with a hazard blocks it; one pick left takes the single
//! flow; and the result records one undo entry, toasts, or logs where it stopped (also for a tab
//! that isn't active).

use super::tests::write_said;
use super::wip_picks_tests::{lists, pick, send};
use crate::effect::{DeleteMode, DiscardEach, Effect, FilesEffect, LocalOp};
use crate::keymap::KeyAction;
use crate::msg::{FailedWrite, Inactive, Msg, WorkFileMsg};
use crate::state::list_keys::ListKey;
use crate::state::wip_lists::PickHow;
use crate::state::{AppState, Modal};
use glowfly_core::repo::{DiffTarget, Hazard};
use glowfly_core::undo::{Action, BatchOp, Discarded};

fn local_op(fx: &[Effect]) -> Option<&LocalOp> {
    fx.iter().find_map(|e| match e {
        Effect::Files(FilesEffect::LocalOp { op, .. }) => Some(op),
        _ => None,
    })
}

fn paths(ps: &[&str]) -> Vec<String> {
    ps.iter().map(|p| (*p).to_owned()).collect()
}

fn wip_key(st: &mut AppState, staged: bool, key: ListKey) -> Vec<Effect> {
    send(st, Msg::Shortcut(KeyAction::WipList { staged, key }))
}

/// a.txt and c.txt (untracked) picked in Unstaged Files, b.txt left out; c.txt open.
fn a_and_c(name: &str) -> AppState {
    let mut st = lists(name);
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    st
}

/// x.txt and z.txt picked in Staged Files, y.txt left out.
fn x_and_z(name: &str) -> AppState {
    let mut st = lists(name);
    send(&mut st, Msg::Repo(crate::msg::RepoMsg::OpenDiff(DiffTarget::Staged("x.txt".into()))));
    pick(&mut st, true, "z.txt", PickHow::Toggle);
    st
}

fn discard(path: &str) -> Action {
    Action::Discard { path: path.into(), what: Discarded::File, before: None, after: None }
}

#[test]
fn space_stages_every_pick_with_gaps() {
    let mut st = a_and_c("picks-space");
    let fx = wip_key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StagePaths(paths(&["a.txt", "c.txt"]))));
}

#[test]
fn space_in_staged_unstages_picks_with_gaps() {
    let mut st = x_and_z("picks-space-staged");
    let fx = wip_key(&mut st, true, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(paths(&["x.txt", "z.txt"]))));
}

#[test]
fn s_key_stages_unstaged_picks_and_u_key_unstages_staged_picks() {
    let mut st = a_and_c("picks-s");
    assert!(local_op(&send(&mut st, Msg::Shortcut(KeyAction::Unstage))).is_none(), "U doesn't fit Unstaged picks");
    let fx = send(&mut st, Msg::Shortcut(KeyAction::Stage));
    assert_eq!(local_op(&fx), Some(&LocalOp::StagePaths(paths(&["a.txt", "c.txt"]))));
    let mut st = x_and_z("picks-u");
    let fx = send(&mut st, Msg::Shortcut(KeyAction::Unstage));
    assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(paths(&["x.txt", "z.txt"]))));
}

#[test]
fn delete_asks_once_naming_the_count() {
    let mut st = a_and_c("picks-delete");
    st.settings.delete_to_trash = true;
    let fx = wip_key(&mut st, false, ListKey::Delete);
    assert!(fx.is_empty());
    let Some(Modal::ConfirmDanger { title, body, button, then, .. }) = &st.modal else { panic!("a danger confirmation") };
    assert_eq!(title, "Discard changes in 2 files");
    assert!(body.starts_with("Discard the changes in 2 files?"), "{body}");
    assert!(body.contains("1 of them isn't tracked by Git"), "{body}");
    assert_eq!(button, "Discard 2 files");
    let want = vec![DiscardEach { path: "a.txt".into(), untracked: false }, DiscardEach { path: "c.txt".into(), untracked: true }];
    assert!(
        matches!(&**then, Msg::WorkFile(WorkFileMsg::DiscardPathsConfirmed { files, mode: DeleteMode::Trash }) if *files == want),
        "in on-screen order, with the settings' mode"
    );
}

#[test]
fn delete_keeps_picks_until_confirmed() {
    let mut st = a_and_c("picks-delete-keep");
    wip_key(&mut st, false, ListKey::Delete);
    assert!(st.modal.is_some());
    assert_eq!(crate::state::wip_lists::picked_paths(&st, false), ["a.txt", "c.txt"]);
}

#[test]
fn confirmed_discard_sends_one_effect_and_clears_picks() {
    let mut st = a_and_c("picks-confirmed");
    let files = vec![DiscardEach { path: "a.txt".into(), untracked: false }, DiscardEach { path: "c.txt".into(), untracked: true }];
    let fx = send(&mut st, Msg::WorkFile(WorkFileMsg::DiscardPathsConfirmed { files: files.clone(), mode: DeleteMode::Permanent }));
    let runs: Vec<_> = fx.iter().filter(|e| matches!(e, Effect::Files(FilesEffect::DiscardPaths { .. }))).collect();
    assert_eq!(runs.len(), 1, "{fx:?}");
    assert!(matches!(runs[0], Effect::Files(FilesEffect::DiscardPaths { files: f, mode: DeleteMode::Permanent, .. }) if *f == files));
    assert!(st.cues.wip_picks.is_none());
}

#[test]
fn discard_with_one_pick_left_uses_the_single_flow() {
    let mut st = a_and_c("picks-one-left");
    send(&mut st, Msg::WorkFile(WorkFileMsg::DiscardPaths(paths(&["a.txt", "gone.txt"]))));
    let Some(Modal::ConfirmDanger { title, then, .. }) = &st.modal else { panic!("the single confirmation") };
    assert_eq!(title, "Discard changes");
    assert!(matches!(&**then, Msg::Stage(crate::msg::StageMsg::DiscardFile(p)) if p == "a.txt"));
}

#[test]
fn a_pick_with_a_hazard_blocks_the_discard() {
    let mut st = a_and_c("picks-hazard");
    let c = st.snap.as_mut().unwrap().unstaged.iter_mut().find(|f| f.path == "c.txt").unwrap();
    c.hazard = Some(Hazard::Twins(vec!["C.txt".into()]));
    wip_key(&mut st, false, ListKey::Delete);
    assert!(st.modal.is_none());
    let last = st.log.last().map(|l| l.text.as_str()).unwrap_or_default();
    assert!(last.starts_with("Can't discard the selected files because of c.txt."), "{last}");
}

#[test]
fn paths_discarded_records_one_undo_entry_and_toasts() {
    let mut st = a_and_c("picks-done");
    let undo = glowfly_core::undo::batch(BatchOp::Discard, vec![discard("a.txt"), discard("b.txt")]);
    send(&mut st, Msg::WorkFile(WorkFileMsg::PathsDiscarded { done: 2, failed: None, undo }));
    assert_eq!(st.session.undo.undo.len(), 1);
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Batch { op: BatchOp::Discard, parts }) if parts.len() == 2));
    let toast = st.toasts.last().map(|t| t.text.as_str()).unwrap_or_default();
    assert!(toast.starts_with("Discarded the changes in 2 files. Undo"), "{toast}");
}

#[test]
fn a_stopped_discard_logs_the_file_and_keeps_the_undo() {
    let mut st = a_and_c("picks-stopped");
    let failed = Some(("c.txt".to_owned(), write_said("in use")));
    send(&mut st, Msg::WorkFile(WorkFileMsg::PathsDiscarded { done: 1, failed, undo: Some(discard("a.txt")) }));
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Discard { path, .. }) if path == "a.txt"));
    let last = st.log.last().map(|l| l.text.as_str()).unwrap_or_default();
    assert!(last.starts_with("Discarded 1 file, then stopped at c.txt: "), "{last}");
    assert!(last.contains("puts back the one discarded"), "{last}");
}

#[test]
fn paths_discarded_for_an_inactive_tab_goes_to_its_history() {
    let undo = glowfly_core::undo::batch(BatchOp::Discard, vec![discard("a.txt"), discard("b.txt")]);
    let ok = WorkFileMsg::PathsDiscarded { done: 2, failed: None, undo: undo.clone() };
    assert!(matches!(ok.on_inactive(), Some(Inactive::Undo(Action::Batch { .. }))));
    let failed = Some(("c.txt".to_owned(), write_said("in use")));
    let stopped = WorkFileMsg::PathsDiscarded { done: 2, failed, undo };
    let Some(Inactive::WriteFailed { at, undo, .. }) = stopped.on_inactive() else { panic!("a failed write") };
    assert_eq!(at, FailedWrite::DiscardPaths { path: "c.txt".into(), done: 2 });
    assert!(undo.is_some());
    assert!(WorkFileMsg::PathsDiscarded { done: 0, failed: None, undo: None }.on_inactive().is_none());
}
