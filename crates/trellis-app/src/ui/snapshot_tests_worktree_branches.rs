//! Branches and worktrees together: a LOCAL row whose branch another worktree has (marked
//! `worktree`), its menu with Checkout and the deletes disabled and Open worktree, the REMOTE and
//! PULL REQUESTS row menus' Create worktree items, and the Create worktree dialog for a remote
//! branch and for a pull request.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_pull_request::with_pr;
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, p, pushed, with_worktrees};
use crate::msg::Msg;
use crate::state::worktrees::WorktreeForm;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::worktree::WorktreeMsg;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;
use trellis_core::worktree_ops::WorktreeBase;

/// The worktrees fixture with `search` checked out in the feature-login worktree.
fn with_search_elsewhere(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_worktrees(theme, name);
    st.snap.as_mut().unwrap().worktrees[1].branch = Some("search".into());
    (st, dir)
}

/// Right-clicks the left panel's row labelled `label`: the topmost one, or the bottom one.
fn row_menu(h: &mut H, label: &str, top: bool) {
    let rows = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 250.0);
    let rect = if top { rows.min_by(|a, b| a.y0.total_cmp(&b.y0)) } else { rows.max_by(|a, b| a.y0.total_cmp(&b.y0)) };
    let rect = rect.expect("the sidebar row");
    click(h, pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32), PointerButton::Secondary);
}

#[test]
fn worktree_local_rows() {
    let (st, dir) = with_search_elsewhere(ThemeChoice::Dark, "worktree-local-rows");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_local_rows");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_local_menu() {
    let (st, dir) = with_search_elsewhere(ThemeChoice::Light, "worktree-local-menu");
    let mut h = harness(st, false);
    h.run();
    row_menu(&mut h, "search", true);
    h.check_snapshot("worktree_local_menu");
    for item in ["Checkout", "Delete…", "Force delete…"] {
        assert!(h.get_by_label(item).is_disabled(), "{item}");
    }
    h.get_by_label("Open worktree").click();
    h.run();
    let login = p("C:/work/app-worktrees/feature-login");
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Open(x)) if *x == login)));

    // A branch no worktree has: Checkout works and there is no Open worktree.
    row_menu(&mut h, "login-page", true);
    assert!(!h.get_by_label("Checkout").is_disabled());
    assert!(h.query_by_label_contains("Open worktree").is_none());
    h.get_by_label("Checkout").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Branch(crate::msg::BranchMsg::Checkout(b)) if b == "login-page")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_remote_menu() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "worktree-remote-menu");
    let snap = st.snap.as_mut().unwrap();
    let tip = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    snap.remotes = vec![("origin".into(), vec![("search".into(), tip)])];
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "search", false);
    h.check_snapshot("worktree_remote_menu");
    h.get_by_label("Create worktree from this branch…").click();
    h.run();
    let want = WorktreeBase::Remote { remote: "origin".into(), branch: "search".into() };
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::CreateDialog(b)) if *b == want)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_pr_menu() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "worktree-pr-menu", true);
    st.pull_request = None;
    let mut h = harness(st, true);
    h.run();
    row_menu(&mut h, "Search results page", true);
    h.check_snapshot("worktree_pr_menu");
    h.get_by_label("Create worktree…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::CreateForPr { id: 12, source }) if source == "search")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_dialogs_for_remote_and_pr() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "worktree-dialog-remote");
    let snap = st.snap.as_mut().unwrap();
    let tip = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    snap.remotes = vec![("origin".into(), vec![("feature".into(), tip)])];
    let base = WorktreeBase::Remote { remote: "origin".into(), branch: "feature".into() };
    let form = WorktreeForm::new(st.snap.as_ref().unwrap(), base);
    assert_eq!(form.branch, "feature");
    st.modal = Some(Modal::NewWorktree(form));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_dialog_remote");
    assert!(h.query_by_label("Create worktree from origin/feature").is_some());
    assert!(!h.get_by_label("Create worktree").is_disabled());
    drop(h);

    let (mut st, _) = with_worktrees(ThemeChoice::Light, "worktree-dialog-remote");
    let base = WorktreeBase::Branch("search".into());
    let form =
        WorktreeForm { title: Some("Create worktree for pull request #12".into()), ..WorktreeForm::new(st.snap.as_ref().unwrap(), base) };
    st.modal = Some(Modal::NewWorktree(form));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("worktree_dialog_pr");
    assert!(h.query_by_label("Create worktree for pull request #12").is_some());
    drop(h);
    remove_fixture(&dir);
}
