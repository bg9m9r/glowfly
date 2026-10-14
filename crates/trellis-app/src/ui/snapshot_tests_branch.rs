//! Branch operations: the rename and upstream dialogs, the local branch menu, and hiding or
//! soloing branches from the left panel. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{harness, reload, remove_fixture, render, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::Msg;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::repo::{BranchInfo, LoadOptions, Tracking};
use trellis_core::settings::{FilterOp, GraphFilter, ThemeChoice};

fn tip(st: &AppState, name: &str) -> Oid {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap()
}

fn branch<'a>(st: &'a mut AppState, name: &str) -> &'a mut BranchInfo {
    st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == name).unwrap()
}

/// `search` tracks origin/search, which exists, and is `behind` it and `ahead` of it.
fn tracked(st: &mut AppState, ahead: usize, behind: usize) {
    let (main, search) = (tip(st, "main"), tip(st, "search"));
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![("main".into(), main), ("search".into(), search)])];
    let b = branch(st, "search");
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "search".into() });
    b.upstream = Some("origin/search".into());
    b.upstream_ref = Some("refs/remotes/origin/search".into());
    (b.ahead, b.behind) = (ahead, behind);
    b.merged = true;
}

#[test]
fn rename_branch_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rename-branch");
    tracked(&mut st, 0, 0);
    st.modal = Some(Modal::RenameBranch { old: "search".into(), name: "login-page".into() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rename_branch_dialog");
    assert!(h.query_by_label_contains("A branch named login-page already exists.").is_some());
    assert!(h.get_by_label("Rename").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn set_upstream_dialog_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "set-upstream");
    let main = tip(&st, "main");
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![("main".into(), main)])];
    st.modal = Some(Modal::SetUpstream { branch: "search".into(), remote: "origin".into(), name: "search".into() });
    render("set_upstream_dialog_light", st, false);
    remove_fixture(&dir);
}

/// Right-clicks the LOCAL row of `search` (the topmost row with that name).
fn open_menu(h: &mut egui_kittest::Harness<'_, (AppState, Vec<Msg>)>) {
    let rect = h.get_all_by_label("search").filter_map(|n| n.bounding_box()).min_by(|a, b| a.y0.total_cmp(&b.y0)).expect("the sidebar row");
    let center = eframe::egui::pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(h, center, PointerButton::Secondary);
}

#[test]
fn branch_menu() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "branch-menu");
    tracked(&mut st, 0, 2);
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    h.check_snapshot("branch_menu");
    h.get_by_label("Fast-forward search to origin/search").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::FastForwardBranch(b) if b == "search")));
    drop(h);

    // Diverged: no fast-forward, and the branch can be deleted together with origin/search.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "branch-menu");
    tracked(&mut st, 1, 2);
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    assert!(h.get_by_label("Fast-forward search to origin/search").is_disabled());
    assert!(h.get_by_label("Push search").is_disabled(), "behind its upstream");
    assert!(h.query_by_label("Delete search and origin/search").is_some_and(|n| !n.is_disabled()));
    assert!(h.query_by_label("Unset upstream").is_some());
    drop(h);

    // Not merged into the checked-out branch: deleting both is disabled.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "branch-menu");
    tracked(&mut st, 0, 0);
    branch(&mut st, "search").merged = false;
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    assert!(h.get_by_label("Delete search and origin/search").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

/// `search` soloed, `login-page` and origin/main hidden, with the graph loaded to match.
fn hidden_solo(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let filter = GraphFilter {
        hidden: vec!["refs/heads/login-page".into(), "refs/remotes/origin/main".into()],
        solo: vec!["refs/heads/search".into()],
    };
    st.settings.set_graph_filter(&workdir, filter.clone());
    let opts = LoadOptions { max_commits: 500, hidden: filter.hidden, solo: filter.solo, ..Default::default() };
    reload(&mut st, &dir, &opts);
    let (main, search) = (tip(&st, "main"), tip(&st, "search"));
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![("main".into(), main), ("search".into(), search)])];
    (st, dir)
}

/// Right-clicks the sidebar row labelled `label`: the topmost, or the bottommost with `last`.
fn row_menu(h: &mut egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, label: &str, last: bool) {
    let mut rows: Vec<_> = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 100.0).collect();
    rows.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    let rect = if last { rows.last() } else { rows.first() }.expect("the sidebar row");
    let center = eframe::egui::pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(h, center, PointerButton::Secondary);
}

fn pushed(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, op: &FilterOp) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::GraphFilter(o) if o == op))
}

#[test]
fn sidebar_hidden_solo() {
    let (st, dir) = hidden_solo("hidden-solo");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("sidebar_hidden_solo");
    assert!(h.query_by_label("Stop soloing").is_some());
    assert!(h.query_by_label("Show all").is_some());
    assert!(h.query_by_label("Show origin/main in graph").is_some());
    h.get_by_label("Show login-page in graph").click();
    h.run();
    assert!(pushed(&h, &FilterOp::Show("refs/heads/login-page".into())));
    h.get_by_label("Stop soloing").click();
    h.run();
    assert!(pushed(&h, &FilterOp::StopSoloing));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_visibility_items() {
    let (st, dir) = hidden_solo("visibility-menu");
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "✔  main", false);
    assert!(h.get_by_label("Hide main in graph").is_disabled(), "the checked-out branch");
    assert!(h.query_by_label("Solo main").is_some());
    drop(h);

    let (st, _) = hidden_solo("visibility-menu");
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "search", false);
    assert_eq!(h.get_all_by_label("Stop soloing search").count(), 2, "the row's marker and the menu item");
    assert!(h.query_by_label("Hide search in graph").is_some_and(|n| !n.is_disabled()));
    drop(h);

    let (st, _) = hidden_solo("visibility-menu");
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "search", true);
    assert!(h.query_by_label("Solo origin/search").is_some());
    h.get_by_label("Hide origin/search in graph").click();
    h.run();
    assert!(pushed(&h, &FilterOp::Hide("refs/remotes/origin/search".into())));
    drop(h);
    remove_fixture(&dir);
}
