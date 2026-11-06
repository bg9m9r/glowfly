//! Headless tests for picking WIP files with Shift+click and Ctrl/Cmd+click (`wip_picks.rs`):
//! picks with gaps, removing one (the open one moves to the nearest pick), Shift ranges from the
//! anchor, what ends the picks (a plain click, the other list, a tab switch), files in a
//! collapsed folder or gone after a status read not counting. The actions on picks are in
//! `wip_picks_actions_tests.rs`.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{LoadMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::file_folds::FileListId;
use crate::state::op_fixture::{rows, snapshot};
use crate::state::wip_lists::{PickHow, cursor, picked, picked_paths};
use glowfly_core::repo::DiffTarget;
use glowfly_core::settings::FileListMode;
use std::time::Instant;

/// Unstaged Files a.txt, b.txt, c.txt (untracked), d.txt; Staged Files x.txt, y.txt, z.txt;
/// a.txt open.
pub(super) fn lists(name: &str) -> AppState {
    let mut st = state();
    let mut snap = snapshot(name);
    snap.unstaged = rows(&[("a.txt", 'M'), ("b.txt", 'M'), ("c.txt", '?'), ("d.txt", 'M')]);
    snap.staged = rows(&[("x.txt", 'M'), ("y.txt", 'M'), ("z.txt", 'A')]);
    st.snap = Some(snap);
    st.session.selection = crate::state::Selection::Wip;
    send(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))));
    st
}

pub(super) fn un(p: &str) -> DiffTarget {
    DiffTarget::Unstaged(p.into())
}

pub(super) fn send(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

pub(super) fn pick(st: &mut AppState, staged: bool, path: &str, how: PickHow) -> Vec<Effect> {
    send(st, Msg::Repo(RepoMsg::PickWipFile { staged, path: path.into(), how }))
}

#[test]
fn ctrl_click_picks_with_gaps() {
    let mut st = lists("picks-gaps");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert_eq!(picked_paths(&st, false), ["a.txt", "c.txt"]);
    assert_eq!(cursor(&st), Some(&un("c.txt")), "the added file opens");
    assert!(picked(&st, false).is_some_and(|p| p.set.contains("a.txt") && !p.set.contains("b.txt")));
}

#[test]
fn ctrl_click_again_removes_and_opens_the_nearest_pick() {
    let mut st = lists("picks-remove");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    pick(&mut st, false, "d.txt", PickHow::Toggle);
    // Not the open file: the diff stays.
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    assert_eq!(picked_paths(&st, false), ["a.txt", "d.txt"]);
    assert_eq!(cursor(&st), Some(&un("d.txt")));
    pick(&mut st, false, "b.txt", PickHow::Toggle);
    // The open one: the nearest pick after it, then before it.
    pick(&mut st, false, "b.txt", PickHow::Toggle);
    assert_eq!(cursor(&st), Some(&un("d.txt")), "after it first");
    pick(&mut st, false, "d.txt", PickHow::Toggle);
    assert_eq!(cursor(&st), Some(&un("a.txt")), "then before it");
    assert!(picked(&st, false).is_none(), "one pick left is no selection");
}

#[test]
fn removing_the_last_pick_ends_the_selection_and_keeps_the_file_open() {
    let mut st = lists("picks-last");
    pick(&mut st, false, "a.txt", PickHow::Toggle);
    assert!(st.cues.wip_picks.is_none());
    assert_eq!(cursor(&st), Some(&un("a.txt")));
}

#[test]
fn shift_click_picks_the_range_from_the_anchor() {
    let mut st = lists("picks-range");
    pick(&mut st, false, "c.txt", PickHow::Range);
    assert_eq!(picked_paths(&st, false), ["a.txt", "b.txt", "c.txt"], "from the open file");
    assert_eq!(cursor(&st), Some(&un("c.txt")));
    pick(&mut st, false, "b.txt", PickHow::Range);
    assert_eq!(picked_paths(&st, false), ["a.txt", "b.txt"], "the anchor stays");
}

#[test]
fn shift_click_after_ctrl_click_ranges_from_the_toggled_file_and_drops_gaps() {
    let mut st = lists("picks-range-after");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    pick(&mut st, false, "d.txt", PickHow::Range);
    assert_eq!(picked_paths(&st, false), ["c.txt", "d.txt"], "a.txt's pick is dropped");
}

#[test]
fn plain_click_on_a_picked_row_ends_the_selection() {
    let mut st = lists("picks-plain");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    send(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))));
    assert!(st.cues.wip_picks.is_none());
    assert!(picked_paths(&st, false).is_empty());
}

#[test]
fn click_in_the_other_list_ends_it() {
    let mut st = lists("picks-other");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    pick(&mut st, true, "y.txt", PickHow::Toggle);
    assert!(picked_paths(&st, false).is_empty());
    assert!(picked(&st, true).is_none(), "one file in Staged Files");
    pick(&mut st, true, "x.txt", PickHow::Toggle);
    assert_eq!(picked_paths(&st, true), ["x.txt", "y.txt"]);
}

#[test]
fn picks_in_a_collapsed_folder_dont_count() {
    let mut st = state();
    let mut snap = snapshot("picks-folded");
    snap.unstaged = rows(&[("a.txt", 'M'), ("src/b.txt", 'M'), ("src/c.txt", 'M')]);
    st.snap = Some(snap);
    st.settings.file_list_mode = FileListMode::Tree;
    send(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("a.txt"))));
    pick(&mut st, false, "src/b.txt", PickHow::Toggle);
    pick(&mut st, false, "src/c.txt", PickHow::Toggle);
    assert_eq!(picked_paths(&st, false).len(), 3);
    st.session.tab_view.file_folds.toggle(FileListId::Unstaged, "src".into());
    assert_eq!(picked_paths(&st, false), ["a.txt"], "the folded files drop out");
    assert!(picked(&st, false).is_none());
}

#[test]
fn status_refresh_prunes_files_that_left() {
    let mut st = lists("picks-prune");
    pick(&mut st, false, "b.txt", PickHow::Toggle);
    pick(&mut st, false, "d.txt", PickHow::Toggle);
    let without_b = rows(&[("a.txt", 'M'), ("c.txt", '?'), ("d.txt", 'M')]);
    let staged = st.snap.as_ref().unwrap().staged.clone();
    let result = Ok(crate::effect::StatusRead::Whole(without_b, staged, git2::RepositoryState::Clean));
    let generation = st.load_gen;
    let msg = LoadMsg::StatusLoaded { generation, result, submodules: None, open_stamp: None, slow: None, lfs_patterns: None, index: None };
    send(&mut st, Msg::Load(msg));
    assert!(!st.cues.wip_picks.as_ref().unwrap().paths.contains("b.txt"), "b.txt left");
    // It comes back: it doesn't rejoin.
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", 'M'), ("b.txt", 'M'), ("c.txt", '?'), ("d.txt", 'M')]);
    assert_eq!(picked_paths(&st, false), ["a.txt", "d.txt"]);
}

#[test]
fn tab_park_clears_picks() {
    let mut st = lists("picks-park");
    pick(&mut st, false, "c.txt", PickHow::Toggle);
    let _ = super::tabs::park(&mut st);
    assert!(st.cues.wip_picks.is_none());
}
