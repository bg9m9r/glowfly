//! A branch and its upstream in the graph: one label with a cloud when they are at the same
//! commit (its tooltip says so), two labels when they aren't, dark and light; the folded label's
//! menu still offers both refs. Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::{Msg, SelectionMsg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::repo::{RefKind, RefLabel};
use trellis_core::settings::ThemeChoice;

/// Where the first chip of a row sits in the BRANCH / TAG column.
const CHIP_X: f32 = 290.0;
/// The fixture's search branch's tip (the WIP row is 0).
const SEARCH_ROW: usize = 1;
const IN_SYNC: &str = "search, in sync with origin/search";

/// The fixture with origin/search (search's upstream) at search's tip when `in_sync`, else
/// further down the history.
fn with_upstream(theme: ThemeChoice, name: &str, in_sync: bool) -> (AppState, PathBuf, Oid) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    let search = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    // Out of sync: the first commit below search's tip that has no labels yet.
    let mut at = search;
    while !in_sync && (at == search || snap.refs.contains_key(&at)) {
        at = snap.commits[snap.row_of[&at]].parents[0];
    }
    let b = snap.locals.iter_mut().find(|b| b.name == "search").unwrap();
    b.upstream = Some("origin/search".into());
    b.upstream_ref = Some("refs/remotes/origin/search".into());
    let label = RefLabel { name: "origin/search".into(), full: "refs/remotes/origin/search".into(), kind: RefKind::Remote, is_head: false };
    let labels = snap.refs.entry(at).or_default();
    labels.push(label);
    labels.sort_by(|a, b| (!a.is_head, a.kind, &a.name).cmp(&(!b.is_head, b.kind, &b.name)));
    snap.remotes = vec![("origin".into(), vec![("search".into(), at)])];
    (st, dir, at)
}

fn labels_named(h: &super::snapshot_tests_worktrees::H, text: &str) -> usize {
    h.query_all_by_label(text).count()
}

#[test]
fn graph_ref_in_sync() {
    let (st, dir, _) = with_upstream(ThemeChoice::Dark, "ref-in-sync", true);
    let mut h = harness(st, true);
    h.run();
    let before = labels_named(&h, IN_SYNC);
    h.input_mut().events.push(Event::PointerMoved(pos2(CHIP_X, row_y(SEARCH_ROW))));
    for _ in 0..4 {
        h.step();
    }
    h.check_snapshot("graph_ref_in_sync");
    assert_eq!(labels_named(&h, IN_SYNC), before + 1, "the tooltip names both refs");
    drop(h);
    let (st, _, _) = with_upstream(ThemeChoice::Light, "ref-in-sync", true);
    render("graph_ref_in_sync_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn graph_ref_out_of_sync() {
    let (st, dir, _) = with_upstream(ThemeChoice::Dark, "ref-out-of-sync", false);
    render("graph_ref_out_of_sync", st, true);
    let (st, _, _) = with_upstream(ThemeChoice::Light, "ref-out-of-sync", false);
    render("graph_ref_out_of_sync_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn graph_ref_in_sync_menu() {
    let (st, dir, search) = with_upstream(ThemeChoice::Dark, "ref-in-sync-menu", true);
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), PointerButton::Secondary);
    assert!(h.query_by_label("Branch search").is_some(), "the branch");
    h.get_by_label("Remote origin/search").hover();
    h.run();
    h.check_snapshot("graph_ref_in_sync_menu");
    h.state_mut().1.clear();
    h.get_by_label("Compare with main").click();
    h.run();
    let compare = h
        .state()
        .1
        .iter()
        .any(|m| matches!(m, Msg::Selection(SelectionMsg::CompareWithHead { oid, name }) if *oid == search && name == "origin/search"));
    assert!(compare, "the remote branch's menu");
    drop(h);
    remove_fixture(&dir);
}
