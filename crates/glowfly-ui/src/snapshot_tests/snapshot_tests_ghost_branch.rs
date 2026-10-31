//! The graph's New branch… ghost button on the checked-out commit: shown while that commit
//! anchors the selection, not for a multi-selection focused elsewhere, gone while a row menu is
//! open or the preference is off, its plus alone in a narrow column; its click and its tooltip
//! with the Create branch keys.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::{BranchMsg, Msg, SelectionMsg};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{ColumnWidths, ThemeChoice};
use std::path::PathBuf;
use std::time::Instant;

const GHOST: &str = "New branch…";
/// main's tip, the checked-out commit (the WIP row is 0).
const HEAD_ROW: usize = 2;

/// The fixture with room after main's chips, and the checked-out commit selected.
fn head_selected(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let head = st.snap.as_ref().and_then(|s| s.head_oid).expect("HEAD");
    st.session.selection = Selection::Commit(head);
    st.settings.column_widths = ColumnWidths { refs: 280.0, ..Default::default() };
    (st, dir)
}

#[test]
fn ghost_button_opens_new_branch() {
    let (st, dir) = head_selected("ghost-click");
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label(GHOST).is_some(), "the head anchors the selection");
    h.get_by_label(GHOST).hover();
    h.run_steps(4);
    h.check_snapshot("graph_ghost_branch_tooltip");
    h.get_by_label(GHOST).click();
    h.run_steps(4);
    let new_branch = |m: &Msg| matches!(m, Msg::Branch(BranchMsg::NewBranch { start: None, .. }));
    assert!(h.state().1.iter().any(new_branch), "a click opens New branch from the current branch");
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(crate::msg::RepoMsg::SelectCommit(_)))), "not a row click");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn ghost_button_hides_off_the_anchor_under_a_menu_and_when_off() {
    // A multi-selection holding the head, focused on another commit.
    let (mut st, dir) = head_selected("ghost-hidden");
    let other = st.snap.as_ref().unwrap().commits[0].oid;
    crate::update::update(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(other)), Instant::now());
    let Selection::Commits(m) = &st.session.selection else { panic!("a multi-selection") };
    assert!(m.focus() != st.snap.as_ref().unwrap().head_oid.unwrap(), "focused elsewhere");
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label(GHOST).is_none(), "the head is only one member of the selection");
    drop(h);

    // A menu open on the head's row.
    let (st, _) = head_selected("ghost-hidden");
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label(GHOST).is_some());
    click(&mut h, pos2(700.0, row_y(HEAD_ROW)), PointerButton::Secondary);
    h.run_steps(2);
    assert!(h.ctx.is_context_menu_open());
    assert!(h.query_by_label(GHOST).is_none(), "gone while the menu is open");
    drop(h);

    // The preference off.
    let (mut st, _) = head_selected("ghost-hidden");
    st.settings.ghost_refs = false;
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label(GHOST).is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_narrow_column_keeps_the_plus_alone() {
    let (mut st, dir) = head_selected("ghost-narrow");
    st.settings.column_widths = ColumnWidths { refs: 190.0, ..Default::default() };
    let mut h = harness(st, true);
    h.run_steps(4);
    let b = h.get_by_label(GHOST).bounding_box().expect("still offered, by its name");
    assert!(b.width() < 30.0, "the label doesn't fit, so the plus shows alone: {b:?}");
    h.check_snapshot("graph_ghost_branch_narrow");
    drop(h);
    remove_fixture(&dir);
}
