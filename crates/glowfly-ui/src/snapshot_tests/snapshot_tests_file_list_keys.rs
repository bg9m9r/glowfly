//! The keyboard focus of the lists that only open files: a click on a row focuses its list, the
//! focused list rings its current row (or, with none, the whole list), each list's plain moves
//! become its messages, and the keys it doesn't use (Delete, Space, Enter, Left and Right, Shift)
//! are left for the bindings, never while a dialog covers it. Shares the fixtures of the views'
//! own snapshot tests.

use super::file_list_keys::focused;
use super::snapshot_tests::{harness, keys_harness, remove_fixture};
use super::snapshot_tests_wip_keys::{H, click_row};
use crate::keymap::KeyAction;
use crate::msg::{Msg, RebaseMsg};
use crate::state::list_keys::{FileList, ListMove};
use crate::state::pull_request::PrTab;
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, Modifiers};
use glowfly_core::rebase::QuickEdit;
use glowfly_core::repo::{self, DiffTarget};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

const DOWN: ListMove = ListMove::Step { down: true };
const UP: ListMove = ListMove::Step { down: false };

/// The file-list moves the frames so far sent.
fn moves(h: &H) -> Vec<(FileList, ListMove)> {
    h.state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Shortcut(KeyAction::FileList { list, mv }) => Some((*list, *mv)),
            _ => None,
        })
        .collect()
}

/// The fixture's commit selected (`with_commit`), with its app.txt diff open.
fn commit_open(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir, oid) = super::snapshot_tests_details_commit::with_commit(ThemeChoice::Dark, name, "Tidy the app");
    let target = DiffTarget::Commit(oid, "app.txt".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    st.session.diff.view = Some((target, diff.into()));
    (st, dir)
}

#[test]
fn commit_files_focused_row() {
    let (st, dir) = commit_open("file-keys-commit-row");
    let mut h = harness(st, true);
    h.run();
    click_row(&mut h, "app.txt");
    assert_eq!(focused(&h.ctx), Some(FileList::Commit), "the click focused the commit's files");
    h.check_snapshot("commit_files_focused_row_dark");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_files_take_their_moves_and_leave_the_rest() {
    let (st, dir) = commit_open("file-keys-commit-moves");
    let mut h = keys_harness(st, true);
    h.run();
    h.press_key(Key::ArrowDown);
    h.run_ok();
    assert!(moves(&h).is_empty(), "no list has the focus");

    click_row(&mut h, "app.txt");
    for key in [Key::ArrowDown, Key::ArrowUp, Key::Home, Key::End, Key::PageDown] {
        h.press_key(key);
    }
    h.run_ok();
    let got = moves(&h);
    assert_eq!(got.len(), 5, "{got:?}");
    let commit = |mv| (FileList::Commit, mv);
    assert_eq!(got[..4], [commit(DOWN), commit(UP), commit(ListMove::Edge { end: false }), commit(ListMove::Edge { end: true })]);
    let (FileList::Commit, ListMove::Page { down: true, rows }) = got[4] else { panic!("{:?}", got[4]) };
    assert!(rows > 1, "a page is the rows in view, less one: {rows}");

    // Keys the list doesn't use reach the bindings: Delete is still Drop commit (while the graph
    // shows, so with the diff closed).
    h.state_mut().0.session.diff.view = None;
    h.run_ok();
    h.state_mut().1.clear();
    for key in [Key::Space, Key::Enter, Key::ArrowLeft, Key::ArrowRight, Key::Delete] {
        h.press_key(key);
    }
    h.press_key_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    h.run_ok();
    assert!(moves(&h).is_empty(), "{:?}", moves(&h));
    let drop_commit = |m: &Msg| matches!(m, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(_))));
    assert!(h.state().1.iter().any(drop_commit), "Delete is Drop commit");
    assert_eq!(focused(&h.ctx), Some(FileList::Commit), "the keys don't move the focus away");

    // Not under a dialog.
    h.state_mut().0.modal = Some(Modal::Shortcuts { filter: String::new() });
    h.run_ok();
    h.state_mut().1.clear();
    h.press_key(Key::ArrowDown);
    h.run_ok();
    assert!(moves(&h).is_empty(), "the sheet covers the list");
    drop(h);
    remove_fixture(&dir);
}

/// A click on the row named `row` focuses `list`, which then sends Down as its move.
fn takes_down(st: AppState, dir: PathBuf, row: &str, list: FileList) {
    let mut h = keys_harness(st, true);
    h.run();
    click_row(&mut h, row);
    assert_eq!(focused(&h.ctx), Some(list), "the click on {row} focused its list");
    h.press_key(Key::ArrowDown);
    h.run_ok();
    assert_eq!(moves(&h), [(list, DOWN)]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn each_file_list_takes_its_moves() {
    let pair = ["Add login page", "Search results page"];
    let (st, dir) = super::snapshot_tests_compare::selected(ThemeChoice::Dark, "file-keys-compare", &pair, false);
    let Selection::Commits(m) = &st.session.selection else { panic!("two commits") };
    let first = m.changes.as_ref().unwrap().as_ref().unwrap().files[0].path.clone();
    takes_down(st, dir, &first, FileList::Compare);

    let stacked = super::snapshot_tests_stacked::STACKED;
    let (st, dir) = super::snapshot_tests_select::with_selection(ThemeChoice::Dark, "file-keys-stacked", &stacked);
    takes_down(st, dir, "README.md", FileList::Stacked);

    let (st, dir) = super::snapshot_tests_compare::compared(ThemeChoice::Dark, "file-keys-workdir", false);
    takes_down(st, dir, "new.txt", FileList::Workdir);

    let (mut st, dir) = super::snapshot_tests_pull_request::with_pr(ThemeChoice::Dark, "file-keys-pr", true);
    super::snapshot_tests_pull_request::compare(&mut st, &dir);
    st.session.pull_request.view.as_mut().unwrap().tab = PrTab::Files;
    takes_down(st, dir, "search.txt", FileList::PullRequest);

    let (st, dir) = super::snapshot_tests_history::with_folder("file-keys-folder", ThemeChoice::Dark);
    takes_down(st, dir, "lexer.rs", FileList::FolderHistory);

    let (mut st, dir) = super::snapshot_tests_conflict_files::merging("file-keys-conflicted", false);
    super::snapshot_tests_conflict_files::fill_lists(&mut st, &dir);
    takes_down(st, dir, "alpha.txt", FileList::Conflicted);

    let (mut st, dir) = super::snapshot_tests_conflict_files::merging("file-keys-resolved", false);
    super::snapshot_tests_conflict_files::fill_lists(&mut st, &dir);
    takes_down(st, dir, "gamma.txt", FileList::Resolved);
}

#[test]
fn conflicted_files_focused_list() {
    // Nothing open in the conflict view: the whole list is ringed.
    let (mut st, dir) = super::snapshot_tests_conflict_files::merging("file-keys-conflicted-list", false);
    super::snapshot_tests_conflict_files::fill_lists(&mut st, &dir);
    let mut h = harness(st, true);
    h.run();
    click_row(&mut h, "alpha.txt");
    assert_eq!(focused(&h.ctx), Some(FileList::Conflicted));
    h.check_snapshot("conflicted_files_focused_list_dark");
    drop(h);
    remove_fixture(&dir);
}
