//! Branch operations: the rename and upstream dialogs and the local branch menu. Shares the
//! fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::Msg;
use crate::state::{AppState, Modal};
use eframe::egui::PointerButton;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::repo::{BranchInfo, Tracking};
use trellis_core::settings::ThemeChoice;

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
}

#[test]
fn rename_branch_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rename-branch");
    tracked(&mut st, 0, 0);
    st.modal = Some(Modal::RenameBranch { old: "search".into(), name: "login-page".into() });
    let mut h = harness(st, true);
    h.run();
    h.snapshot("rename_branch_dialog");
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
    h.snapshot("branch_menu");
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
    assert!(h.query_by_label("Delete search and origin/search").is_some());
    assert!(h.query_by_label("Unset upstream").is_some());
    drop(h);
    remove_fixture(&dir);
}
