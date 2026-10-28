//! Branch operations: the new branch, rename and upstream dialogs, the local branch menu, and
//! hiding or soloing branches from the left panel. Shares the fixture repository and renderer
//! with `snapshot_tests`.

use super::snapshot_tests::{harness, reload, remove_fixture, render, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::BranchMsg;
use crate::msg::GraphMsg;
use crate::msg::Msg;
use crate::state::touched::Touched;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::{BranchInfo, LoadOptions, Tracking};
use glowfly_core::settings::{FilterOp, GraphFilter, ThemeChoice};

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

/// A form whose name field was typed in, so its problem shows.
pub(in crate::ui) fn typed() -> Touched {
    let mut t = Touched::default();
    t.mark("name");
    t
}

#[test]
fn rename_branch_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rename-branch");
    tracked(&mut st, 0, 0);
    st.modal = Some(Modal::RenameBranch { old: "search".into(), name: "login-page".into(), touched: typed() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rename_branch_dialog");
    assert!(h.query_by_label_contains("A branch named login-page already exists.").is_some());
    assert!(h.get_by_label("Rename").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

/// A folder of the new name exists in another case: on Windows and macOS the branch would land
/// in it, so the name is refused on every OS.
#[test]
fn new_branch_dialog_folder_case() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-branch-folder-case");
    branch(&mut st, "search").name = "feature/search".into();
    st.modal = Some(Modal::NewBranch { start: None, name: "Feature/login".into(), checkout: true, touched: typed() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("new_branch_dialog_folder_case");
    assert!(h.query_by_label_contains("The folder Feature already exists as feature; use that spelling.").is_some());
    assert!(h.get_by_label("Create").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn set_upstream_dialog_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "set-upstream");
    let main = tip(&st, "main");
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![("main".into(), main)])];
    st.modal =
        Some(Modal::SetUpstream { branch: "search".into(), remote: "origin".into(), name: "search".into(), touched: Default::default() });
    render("set_upstream_dialog_light", st, false);
    remove_fixture(&dir);
}

/// A LOCAL row ahead of and behind its upstream: each count with a painted arrow.
#[test]
fn sidebar_ahead_behind() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "ahead-behind");
    tracked(&mut st, 2, 3);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("sidebar_ahead_behind");
    // The counts are painted beside the name, not part of the row's accessible name.
    assert!(h.get_all_by_label("search").next().is_some());
    remove_fixture(&dir);
}

#[test]
fn sidebar_ahead_only_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "ahead-only");
    tracked(&mut st, 1, 0);
    render("sidebar_ahead_only_light", st, false);
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
    h.get_by_label("Fast-forward to origin/search").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Branch(BranchMsg::FastForwardBranch(b)) if b == "search")));
    open_menu(&mut h);
    h.get_by_label("Copy link").click();
    h.run();
    let want = crate::deep_link::LinkTarget::LocalBranch("search".into());
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::DeepLink(crate::deep_link::DeepLinkMsg::Copy(t)) if *t == want)));
    drop(h);

    // Diverged: no fast-forward, and the branch can be deleted together with origin/search.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "branch-menu");
    tracked(&mut st, 1, 2);
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    assert!(h.get_by_label("Fast-forward to origin/search").is_disabled());
    assert!(h.get_by_label("Push").is_disabled(), "behind its upstream");
    assert!(h.query_by_label("Delete together with origin/search…").is_some_and(|n| !n.is_disabled()));
    assert!(h.query_by_label("Unset upstream").is_some());
    drop(h);

    // Not merged into the checked-out branch: deleting both is disabled.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "branch-menu");
    tracked(&mut st, 0, 0);
    branch(&mut st, "search").merged = false;
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h);
    assert!(h.get_by_label("Delete together with origin/search…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

/// `search` soloed, `login-page` and origin/main hidden, with the graph loaded to match.
fn hidden_solo(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let workdir = st.snap.as_ref().unwrap().real_workdir.clone();
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
    h.state().1.iter().any(|m| matches!(m, Msg::Graph(GraphMsg::GraphFilter(o)) if o == op))
}

#[test]
fn sidebar_hidden_solo() {
    let (st, dir) = hidden_solo("hidden-solo");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("sidebar_hidden_solo");
    assert!(h.query_by_label("Show the rest").is_some());
    assert!(h.query_by_label("Show all").is_some());
    assert!(h.query_by_label("Show origin/main in graph").is_some());
    h.get_by_label("Show login-page in graph").click();
    h.run();
    assert!(pushed(&h, &FilterOp::Show("refs/heads/login-page".into())));
    h.get_by_label("Show the rest").click();
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
    assert!(h.get_by_label("Hide in graph").is_disabled(), "the checked-out branch");
    assert!(h.query_by_label("Show only this").is_some());
    drop(h);

    let (st, _) = hidden_solo("visibility-menu");
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "search", false);
    assert_eq!(h.get_all_by_label("Stop showing only search").count(), 1, "the row's marker");
    assert_eq!(h.get_all_by_label("Stop showing only this").count(), 1, "the menu item");
    assert_eq!(h.get_all_by_label("Show the rest").count(), 1, "the banner's button");
    assert!(h.query_by_label("Hide in graph").is_some_and(|n| !n.is_disabled()));
    drop(h);

    let (st, _) = hidden_solo("visibility-menu");
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "search", true);
    assert!(h.query_by_label("Show only this").is_some());
    h.get_by_label("Hide in graph").click();
    h.run();
    assert!(pushed(&h, &FilterOp::Hide("refs/remotes/origin/search".into())));
    drop(h);
    remove_fixture(&dir);
}
