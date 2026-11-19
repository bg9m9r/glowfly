//! Compare with the current branch in the local branch menu and the tag menu: the row after
//! Check for conflicts (left out on the checked-out branch), greyed when the branch or tag is at
//! HEAD's commit, and on a detached HEAD only the tag menu shows it, as "the current branch".

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use super::snapshot_tests_tag_menu::tag_row;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{Msg, SelectionMsg};
use glowfly_model::state::AppState;

const DATE: &str = "2026-01-05T10:00:00+00:00";

fn tip(st: &AppState, name: &str) -> Oid {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap()
}

fn compared(h: &H, oid: Oid, name: &str) -> bool {
    pushed(h, &|m| matches!(m, Msg::Selection(SelectionMsg::CompareWithHead { oid: o, name: n }) if *o == oid && n == name))
}

fn right_click(h: &mut H, label: &str) {
    let at = tag_row(h, label);
    click(h, at, PointerButton::Secondary);
}

#[test]
fn branch_menu_compare_with_head() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "cmp-head-branch");
    let search = tip(&st, "search");
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "search");
    assert!(!h.get_by_label("Compare with main").is_disabled());
    h.get_by_label("Compare with main").click();
    h.run();
    assert!(compared(&h, search, "search"));

    right_click(&mut h, "main, checked out");
    assert!(h.query_by_label("Compare with main").is_none(), "not offered on the checked-out branch");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_compare_same_commit() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "cmp-head-same");
    git(&dir, &["branch", "release", "main"], DATE);
    reload(&mut st, &dir, &LoadOptions::all(500));
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "release");
    assert!(h.get_by_label("Compare with main").is_disabled());
    h.check_snapshot("branch_menu_compare_same_commit");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tag_menu_compare_with_head() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "cmp-head-tag");
    git(&dir, &["tag", "-a", "v0.2.0", "-m", "Second search", "search"], DATE);
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.view.sections.tags = true;
    let search = tip(&st, "search");
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "v0.2.0");
    assert!(!h.get_by_label("Compare with main").is_disabled());
    h.check_snapshot("tag_menu_compare");
    h.get_by_label("Compare with main").click();
    h.run();
    assert!(compared(&h, search, "v0.2.0"), "the tag's commit, not the tag object");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tag_menu_compare_detached() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "cmp-head-detached");
    git(&dir, &["checkout", "-q", "--detach"], DATE);
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.view.sections.tags = true;
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "v0.1.0");
    assert!(h.get_by_label("Compare with the current branch").is_disabled());
    assert!(h.query_by_label("Compare with main").is_none());
    drop(h);
    remove_fixture(&dir);
}
