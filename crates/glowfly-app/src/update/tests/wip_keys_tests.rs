//! Headless tests for the keys of a focused Unstaged or Staged Files list: Page Up/Page Down and
//! Home/End, Shift moves selecting files that Space then stages or unstages together, Space on
//! one file, Enter, Delete through the discard confirmation (and its undo), Left/Right to the
//! other list keeping the place, and the Stage and Unstage buttons moving on from the open file.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::keymap::KeyAction;
use crate::msg::{JobMsg, Msg, RepoMsg, StageMsg};
use crate::state::list_keys::{ListKey, ListMove};
use crate::state::wip_lists::span_paths;
use crate::state::{AppState, Modal};
use glowfly_core::repo::DiffTarget;
use glowfly_core::undo::Action;
use std::path::PathBuf;
use std::time::Instant;

const DOWN: ListMove = ListMove::Step { down: true };
const UP: ListMove = ListMove::Step { down: false };

/// A repository with a.txt modified and b.txt to e.txt new, opened, with a.txt's diff open.
fn five_files(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    for f in ["b", "c", "d", "e"] {
        std::fs::write(dir.join(format!("{f}.txt")), format!("{f}\n")).unwrap();
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))));
    (st, dir)
}

fn un(path: &str) -> DiffTarget {
    DiffTarget::Unstaged(path.into())
}

fn sd(path: &str) -> DiffTarget {
    DiffTarget::Staged(path.into())
}

fn key(st: &mut AppState, staged: bool, key: ListKey) -> Vec<Effect> {
    update(st, Msg::Shortcut(KeyAction::WipList { staged, key }), Instant::now())
}

/// Sends the key and runs what it starts, as the app would.
fn press(st: &mut AppState, staged: bool, key: ListKey) {
    drive(st, Msg::Shortcut(KeyAction::WipList { staged, key }));
}

/// The diff `fx` opens.
fn opened(fx: &[Effect]) -> Option<&DiffTarget> {
    fx.iter().find_map(|e| match e {
        Effect::LoadDiff { target, reload: false, .. } => Some(target),
        _ => None,
    })
}

fn local_op(fx: &[Effect]) -> Option<&LocalOp> {
    fx.iter().find_map(|e| match e {
        Effect::LocalOp { op, .. } => Some(op),
        _ => None,
    })
}

fn shown(st: &AppState) -> Option<&DiffTarget> {
    st.session.diff.view.as_ref().map(|(t, _)| t)
}

fn staged_paths(st: &AppState) -> Vec<&str> {
    st.snap.as_ref().unwrap().staged.iter().map(|f| f.path.as_str()).collect()
}

#[test]
fn page_and_edge_keys_move_by_a_page_and_to_the_ends() {
    let (mut st, dir) = five_files("wip-keys-page");
    let page = |down| ListKey::Move(ListMove::Page { down, rows: 3 });
    let fx = key(&mut st, false, page(true));
    assert_eq!(opened(&fx), Some(&un("d.txt")));
    let fx = key(&mut st, false, page(true));
    assert_eq!(opened(&fx), Some(&un("e.txt")), "a page stops at the last file");
    assert!(key(&mut st, false, page(true)).is_empty(), "and stays there");
    let fx = key(&mut st, false, ListKey::Move(ListMove::Edge { end: false }));
    assert_eq!(opened(&fx), Some(&un("a.txt")));
    assert!(key(&mut st, false, page(false)).is_empty());
    let fx = key(&mut st, false, ListKey::Move(ListMove::Edge { end: true }));
    assert_eq!(opened(&fx), Some(&un("e.txt")));
    // From a file of the other list, the first file of this one.
    let fx = key(&mut st, true, ListKey::Move(ListMove::Edge { end: true }));
    assert!(fx.is_empty(), "the Staged list is empty");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shift_moves_select_files_and_space_stages_them_together() {
    let (mut st, dir) = five_files("wip-keys-span");
    press(&mut st, false, ListKey::Move(DOWN));
    press(&mut st, false, ListKey::Extend(DOWN));
    press(&mut st, false, ListKey::Extend(DOWN));
    assert_eq!(span_paths(&st, false), ["b.txt", "c.txt", "d.txt"]);
    press(&mut st, false, ListKey::Extend(UP));
    assert_eq!(span_paths(&st, false), ["b.txt", "c.txt"], "back towards the start");
    press(&mut st, false, ListKey::Extend(DOWN));
    assert_eq!(shown(&st), Some(&un("d.txt")), "the moving end is open");

    let fx = key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StagePaths(vec!["b.txt".into(), "c.txt".into(), "d.txt".into()])), "{fx:?}");
    assert!(st.view.wip_span.is_none());
    for e in fx {
        if let Some(m) = crate::runner::execute(e) {
            drive(&mut st, m);
        }
    }
    assert_eq!(staged_paths(&st), ["b.txt", "c.txt", "d.txt"]);
    assert_eq!(shown(&st), Some(&un("e.txt")), "on to the file after the selection");
    assert!(st.log.iter().all(|l| l.ok), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());

    // And unstaged together from the Staged list, moving on to the file before them.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("c.txt"))));
    press(&mut st, true, ListKey::Extend(DOWN));
    assert_eq!(span_paths(&st, true), ["c.txt", "d.txt"]);
    press(&mut st, true, ListKey::Toggle);
    assert_eq!(staged_paths(&st), ["b.txt"]);
    assert_eq!(shown(&st), Some(&sd("b.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plain_move_or_a_click_ends_the_selection() {
    let (mut st, dir) = five_files("wip-keys-span-end");
    press(&mut st, false, ListKey::Extend(DOWN));
    assert_eq!(span_paths(&st, false), ["a.txt", "b.txt"]);
    press(&mut st, false, ListKey::Move(DOWN));
    assert!(span_paths(&st, false).is_empty());
    press(&mut st, false, ListKey::Extend(UP));
    assert_eq!(span_paths(&st, false), ["b.txt", "c.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("e.txt"))));
    assert!(span_paths(&st, false).is_empty(), "a click elsewhere");
    // Space then acts on the open file alone.
    let fx = key(&mut st, false, ListKey::Toggle);
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("e.txt".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn space_stages_or_unstages_the_open_file_and_moves_on() {
    let (mut st, dir) = five_files("wip-keys-space");
    press(&mut st, false, ListKey::Toggle);
    assert_eq!(staged_paths(&st), ["a.txt"]);
    assert_eq!(shown(&st), Some(&un("b.txt")));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("a.txt"))));
    press(&mut st, true, ListKey::Toggle);
    assert!(staged_paths(&st).is_empty());
    // Space in a list the open file isn't in does nothing.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("c.txt"))));
    assert!(key(&mut st, true, ListKey::Toggle).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_stage_and_unstage_buttons_move_on_from_the_open_file() {
    let (mut st, dir) = five_files("wip-keys-buttons");
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    assert_eq!(shown(&st), Some(&un("b.txt")), "the row's Stage on the open file");
    // Another row's button leaves the open file open.
    drive(&mut st, Msg::Stage(StageMsg::StageFile("d.txt".into())));
    assert_eq!(shown(&st), Some(&un("b.txt")));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("a.txt"))));
    drive(&mut st, Msg::Stage(StageMsg::UnstageFile("a.txt".into())));
    assert_eq!(shown(&st), Some(&sd("d.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn enter_opens_where_the_list_was_left_else_its_first_file() {
    let (mut st, dir) = five_files("wip-keys-enter");
    git(&dir, &["add", "b.txt", "c.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("d.txt"))));
    // Enter in the list the open file is in: nothing to open.
    assert!(key(&mut st, false, ListKey::Open).is_empty());
    let fx = key(&mut st, true, ListKey::Open);
    assert_eq!(opened(&fx), Some(&sd("b.txt")), "the Staged list's first file");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("c.txt"))));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))));
    let fx = key(&mut st, true, ListKey::Open);
    assert_eq!(opened(&fx), Some(&sd("c.txt")), "where it was left");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_asks_to_discard_and_undo_brings_the_file_back() {
    let (mut st, dir) = five_files("wip-keys-delete");
    press(&mut st, false, ListKey::Delete);
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { .. })), "the menu's confirmation asks first");
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\ntwo\n", "nothing changed yet");
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(!std::fs::read_to_string(dir.join("a.txt")).unwrap().contains("two"), "back to the index version");
    assert!(matches!(st.session.undo.undo.last(), Some(Action::Discard { path, .. }) if path == "a.txt"));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\ntwo\n");

    // An untracked file asks with its own wording; the Staged list has no Delete.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("b.txt"))));
    press(&mut st, false, ListKey::Delete);
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { .. })));
    st.modal = None;
    git(&dir, &["add", "c.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("c.txt"))));
    assert!(key(&mut st, true, ListKey::Delete).is_empty());
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn left_and_right_go_to_the_other_list_keeping_the_place() {
    let (mut st, dir) = five_files("wip-keys-across");
    // a.txt partly staged; c.txt staged.
    git(&dir, &["add", "a.txt", "c.txt"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))));
    assert!(key(&mut st, false, ListKey::Across { right: false }).is_empty(), "Left in the Unstaged list");
    assert_eq!(st.view.wip_focus, None);
    // The same file's other half.
    let fx = key(&mut st, false, ListKey::Across { right: true });
    assert_eq!(opened(&fx), Some(&sd("a.txt")));
    assert_eq!(st.view.wip_focus, Some(true), "the Staged list takes the keyboard");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("c.txt"))));
    // c.txt isn't unstaged: back where the Unstaged list was left.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("d.txt"))));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(sd("c.txt"))));
    let fx = key(&mut st, true, ListKey::Across { right: false });
    assert_eq!(opened(&fx), Some(&un("d.txt")));
    assert_eq!(st.view.wip_focus, Some(false));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn going_to_an_empty_list_only_moves_the_keyboard() {
    let (mut st, dir) = five_files("wip-keys-across-empty");
    let fx = key(&mut st, false, ListKey::Across { right: true });
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.view.wip_focus, Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}
