//! A branch and its upstream in the graph: one label with a cloud when they are at the same
//! commit (its tooltip says so), two labels when they aren't, dark and light; the folded label's
//! menu still offers both refs. Copies on other remotes at the same commit fold in too, and
//! copies on several remotes with no local branch share one remote label. Shares the fixture repository with `snapshot_tests`.

use super::menu_testing::open_submenu;
use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::{Msg, SelectionMsg};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::{RefKind, RefLabel};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// Where the first chip of a row sits in the BRANCH / TAG column.
const CHIP_X: f32 = 290.0;
/// The fixture's search branch's tip (the WIP row is 0).
const SEARCH_ROW: usize = 1;
const IN_SYNC: &str = "search, in sync with origin/search";

/// The fixture with origin/search (search's upstream) at search's tip when `in_sync`, else
/// further down the history.
pub(in crate::ui) fn with_upstream(theme: ThemeChoice, name: &str, in_sync: bool) -> (AppState, PathBuf, Oid) {
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
    let labels = snap.refs.entry(at).or_default();
    labels.push(remote_label("origin/search"));
    sort_labels(labels);
    snap.remotes = vec![("origin".into(), vec![("search".into(), at)])];
    (st, dir, at)
}

/// `with_upstream` in sync, with a copy of search on remote fork at the same commit too.
fn with_copies(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir, at) = with_upstream(theme, name, true);
    let snap = st.snap.as_mut().unwrap();
    let labels = snap.refs.get_mut(&at).unwrap();
    labels.push(remote_label("fork/search"));
    sort_labels(labels);
    snap.remotes.push(("fork".into(), vec![("search".into(), at)]));
    (st, dir)
}

fn remote_label(name: &str) -> RefLabel {
    RefLabel { name: name.into(), full: format!("refs/remotes/{name}"), kind: RefKind::Remote, is_head: false }
}

/// The snapshot's label order: HEAD's first, then by kind and name.
fn sort_labels(labels: &mut [RefLabel]) {
    labels.sort_by(|a, b| (!a.is_head, a.kind, &a.name).cmp(&(!b.is_head, b.kind, &b.name)));
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
    open_submenu(&mut h, "Remote origin/search");
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

#[test]
fn graph_ref_copies_in_sync() {
    const COPIES: &str = "search, in sync with origin/search and fork/search";
    let (st, dir) = with_copies(ThemeChoice::Dark, "ref-copies-in-sync");
    let mut h = harness(st, true);
    h.run();
    let before = labels_named(&h, COPIES);
    h.input_mut().events.push(Event::PointerMoved(pos2(CHIP_X, row_y(SEARCH_ROW))));
    for _ in 0..4 {
        h.step();
    }
    h.check_snapshot("graph_ref_copies_in_sync");
    assert_eq!(labels_named(&h, COPIES), before + 1, "the tooltip names every copy");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_ref_remote_copies_tooltip() {
    const COPIES: &str = "origin/review, in sync with fork/review";
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "ref-remote-copies");
    let snap = st.snap.as_mut().unwrap();
    let search = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    let labels = snap.refs.entry(search).or_default();
    labels.extend([remote_label("fork/review"), remote_label("origin/review")]);
    sort_labels(labels);
    snap.remotes = vec![("fork".into(), vec![("review".into(), search)]), ("origin".into(), vec![("review".into(), search)])];
    st.settings.column_widths.refs = super::snapshot_tests_graph_double_click::WIDE_REFS;
    let mut h = harness(st, true);
    h.run();
    let before = labels_named(&h, COPIES);
    // The row's chips are [search, origin/review]; the second sits at about x 348-456.
    h.input_mut().events.push(Event::PointerMoved(pos2(390.0, row_y(SEARCH_ROW))));
    for _ in 0..4 {
        h.step();
    }
    assert_eq!(labels_named(&h, COPIES), before + 1, "the remote chip's tooltip names the other copy");
    drop(h);
    remove_fixture(&dir);
}
