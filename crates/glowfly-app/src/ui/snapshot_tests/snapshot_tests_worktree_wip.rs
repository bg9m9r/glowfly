//! Other worktrees' WIP rows in the graph: rows above the HEAD commits of changed worktrees (two
//! at one commit, a locked one above HEAD, none for a missing one), dark and light; a click
//! opens the worktree without selecting anything, and Down skips the rows.

use super::snapshot_tests::{harness, remove_fixture, render};
use super::snapshot_tests_select::{click, row_y};
use super::snapshot_tests_worktrees::{p, with_worktrees};
use crate::msg::{Msg, RepoMsg};
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::{AppState, Selection};
use crate::worktree::WorktreeMsg;
use eframe::egui::{Key, PointerButton, pos2};
use glowfly_core::repo::ChangeCounts;
use glowfly_core::settings::ResizableColumn::{Commit, Date, Refs};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

fn counts(modified: usize, added: usize, deleted: usize) -> ChangeCounts {
    ChangeCounts { modified, added, deleted }
}

/// The shared worktrees fixture with changes in feature-login and hotfix (at commits[2]), old
/// (locked, at HEAD) and gone (missing, so it gets no row), laid out again.
pub(in crate::ui) fn with_wips(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_worktrees(theme, name);
    // Narrower side columns leave the message column room for the labels and counts.
    st.settings.column_widths = st.settings.column_widths.with(Refs, 110.0).with(Commit, 100.0).with(Date, 140.0);
    let snap = st.snap.as_mut().unwrap();
    for (i, c) in [(1, counts(2, 1, 0)), (2, counts(0, 0, 3)), (3, counts(1, 0, 0)), (4, counts(1, 0, 0))] {
        snap.worktrees[i].changes = c;
    }
    snap.lay_out_graph();
    (st, dir)
}

/// The worktree (index into `snap.worktrees`) shown in display row `row`, if it is a worktree row.
fn worktree_in(st: &AppState, row: usize) -> Option<usize> {
    let snap = st.snap.as_ref().unwrap();
    match GraphRows::new(snap, &st.session.search, false).at(row)? {
        DisplayRow::Worktree(k) => Some(snap.graph.worktree_wips[k].worktree),
        _ => None,
    }
}

/// The display row of commit `commits[i]`.
pub(in crate::ui) fn commit_row(st: &AppState, i: usize) -> usize {
    let snap = st.snap.as_ref().unwrap();
    GraphRows::new(snap, &st.session.search, false).row_of(snap.commits[i].oid).unwrap()
}

#[test]
fn graph_worktree_wips() {
    let (st, dir) = with_wips(ThemeChoice::Dark, "worktree-wips");
    let snap = st.snap.as_ref().unwrap();
    let head = snap.row_of[&snap.head_oid.unwrap()];
    assert_eq!(snap.graph.worktree_wips.len(), 3, "gone is missing: no row");
    assert_eq!(worktree_in(&st, commit_row(&st, head) - 1), Some(3), "old's row sits above HEAD");
    let at = commit_row(&st, 2);
    assert_eq!((worktree_in(&st, at - 2), worktree_in(&st, at - 1)), (Some(1), Some(2)), "login, then hotfix");
    assert_eq!(GraphRows::new(snap, &st.session.search, false).at(0), Some(DisplayRow::Wip));
    render("graph_worktree_wips", st, true);
    let (st, _) = with_wips(ThemeChoice::Light, "worktree-wips");
    render("graph_worktree_wips_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn clicking_a_worktree_wip_row_opens_it() {
    let (st, dir) = with_wips(ThemeChoice::Dark, "worktree-wip-click");
    let row = commit_row(&st, 2) - 2;
    assert_eq!(worktree_in(&st, row), Some(1));
    let mut h = harness(st, true);
    h.run();
    h.state_mut().1.clear();
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Primary);
    let msgs = &h.state().1;
    let login = p("C:/work/app-worktrees/feature-login");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Worktree(WorktreeMsg::Open(x)) if *x == login)));
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_) | RepoMsg::SelectWip))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn arrow_down_skips_worktree_wip_rows() {
    let (mut st, dir) = with_wips(ThemeChoice::Dark, "worktree-wip-keys");
    let (one, two) = {
        let snap = st.snap.as_ref().unwrap();
        (snap.commits[1].oid, snap.commits[2].oid)
    };
    assert_eq!(commit_row(&st, 2) - commit_row(&st, 1), 3, "two worktree rows between them");
    st.session.selection = Selection::Commit(one);
    let mut h = harness(st, true);
    h.run_steps(4);
    h.state_mut().1.clear();
    h.press_key(Key::ArrowDown);
    h.run_steps(4);
    let selected: Vec<_> =
        h.state().1.iter().filter_map(|m| if let Msg::Repo(RepoMsg::SelectCommit(o)) = m { Some(*o) } else { None }).collect();
    assert_eq!(selected, [two]);
    drop(h);
    remove_fixture(&dir);
}
