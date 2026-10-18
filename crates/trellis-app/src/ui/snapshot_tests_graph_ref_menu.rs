//! Right-clicking a graph row's BRANCH / TAG chips: the ref's own menu, a submenu per ref when
//! the row has several (a remote branch's among them), and the commit menu elsewhere on the
//! row. Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::{Msg, SelectionMsg};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use trellis_core::repo::{RefKind, RefLabel};
use trellis_core::settings::ThemeChoice;

/// Where the first chip of a row sits in the BRANCH / TAG column.
const CHIP_X: f32 = 290.0;
/// The fixture's rows (the WIP row is 0): the search branch's tip and the merge on main.
const SEARCH_ROW: usize = 1;
const MAIN_ROW: usize = 2;

#[test]
fn graph_label_menu_branch() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "label-menu-branch");
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), PointerButton::Secondary);
    h.check_snapshot("graph_label_menu_branch");
    assert!(h.query_by_label("Merge search into main").is_some(), "the branch's own menu");
    assert!(h.query_by_label("Create patch…").is_none(), "not the commit menu");
    h.state_mut().1.clear();
    h.get_by_label("Delete search").click();
    h.run();
    let confirm = h.state().1.iter().any(|m| matches!(m, Msg::Confirm { title, .. } if title == "Delete branch"));
    assert!(confirm, "the confirmation is asked for");

    // Elsewhere on the row, the commit menu.
    click(&mut h, pos2(700.0, row_y(SEARCH_ROW)), PointerButton::Secondary);
    assert!(h.query_by_label("Create patch…").is_some());
    assert!(h.query_by_label("Delete search").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_label_menu_many() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "label-menu-many");
    let mut h = harness(st, true);
    h.run();
    // main's tip carries the main branch and the v0.1.0 tag: a submenu each.
    click(&mut h, pos2(CHIP_X, row_y(MAIN_ROW)), PointerButton::Secondary);
    h.check_snapshot("graph_label_menu_many");
    assert!(h.query_by_label("Branch main").is_some());
    h.get_by_label("Tag v0.1.0").hover();
    h.run();
    h.check_snapshot("graph_label_menu_tag");
    assert!(h.query_by_label("Checkout (detached)").is_some(), "the tag's menu");
    h.state_mut().1.clear();
    h.get_by_label("Delete locally").click();
    h.run();
    let confirm = h.state().1.iter().any(|m| matches!(m, Msg::Confirm { title, .. } if title == "Delete tag"));
    assert!(confirm, "the confirmation is asked for");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_label_menu_remote() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "label-menu-remote");
    let snap = st.snap.as_mut().unwrap();
    let search = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    let main = snap.locals.iter().find(|b| b.name == "main").and_then(|b| b.oid).unwrap();
    let label = RefLabel { name: "origin/search".into(), full: "refs/remotes/origin/search".into(), kind: RefKind::Remote, is_head: false };
    snap.refs.entry(search).or_default().push(label);
    snap.remotes = vec![("origin".into(), vec![("main".into(), main), ("search".into(), search)])];
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), PointerButton::Secondary);
    h.get_by_label("Remote origin/search").hover();
    h.run();
    h.check_snapshot("graph_label_menu_remote");
    assert!(h.query_by_label("Compare origin/search with main").is_some(), "the remote branch's menu");
    h.state_mut().1.clear();
    h.get_by_label("Compare origin/search with main").click();
    h.run();
    let compare = h
        .state()
        .1
        .iter()
        .any(|m| matches!(m, Msg::Selection(SelectionMsg::CompareWithHead { oid, name }) if *oid == search && name == "origin/search"));
    assert!(compare, "the pair is asked for");
    drop(h);
    remove_fixture(&dir);
}
