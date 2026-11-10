//! Right-clicking a graph row's BRANCH / TAG chips: the ref's own menu, a submenu per ref when
//! the row has several (a remote branch's among them), and the commit menu elsewhere on the
//! row; a tag's push items, the same as in the TAGS menu; and a branch's Merge, Rebase and
//! preview naming the branch, not a same-named tag. Shares the fixture repository with
//! `snapshot_tests`.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{harness, reload, remove_fixture, with_repo};
use super::snapshot_tests_remotes::{click_steps, running};
use super::snapshot_tests_select::{click, row_y};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::branch_ops::{merge_cmd, rebase_cmd};
use glowfly_core::branch_ref::BranchRef;
use glowfly_core::op_preview::PreviewOp;
use glowfly_core::repo::{LoadOptions, RefKind, RefLabel};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{JobMsg, Msg, OpPreviewMsg, SelectionMsg};
use glowfly_model::tag::TagMsg;

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
    assert!(h.query_by_label("Merge into main").is_some(), "the branch's own menu");
    assert!(h.query_by_label("Create patch…").is_none(), "not the commit menu");
    h.state_mut().1.clear();
    h.get_by_label("Delete…").click();
    h.run();
    let dialog = h.state().1.iter().any(|m| matches!(m, Msg::Branch(glowfly_model::msg::BranchMsg::DeleteDialog(b)) if b == "search"));
    assert!(dialog, "the Delete dialog opens");

    // Elsewhere on the row, the commit menu.
    click(&mut h, pos2(700.0, row_y(SEARCH_ROW)), PointerButton::Secondary);
    assert!(h.query_by_label("Create patch…").is_some());
    assert!(h.query_by_label("Merge into main").is_none());
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
    open_submenu(&mut h, "Tag v0.1.0");
    h.check_snapshot("graph_label_menu_tag");
    assert!(h.query_by_label("Checkout (detached)").is_some(), "the tag's menu");
    h.state_mut().1.clear();
    h.get_by_label("Delete…").click();
    h.run();
    let opens = h.state().1.iter().any(|m| matches!(m, Msg::Tag(TagMsg::OpenDelete { tag }) if tag == "v0.1.0"));
    assert!(opens, "the Delete tag dialog is asked for");
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
    open_submenu(&mut h, "Remote origin/search");
    h.check_snapshot("graph_label_menu_remote");
    assert!(h.query_by_label("Compare with main").is_some(), "the remote branch's menu");
    h.state_mut().1.clear();
    h.get_by_label("Compare with main").click();
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

/// Opens the tag's submenu on main's tip, whose chips carry main and v0.1.0, and in it the Push
/// to submenu.
fn open_tag_submenu(h: &mut super::snapshot_tests_worktrees::H) {
    click(h, pos2(CHIP_X, row_y(MAIN_ROW)), PointerButton::Secondary);
    open_submenu(h, "Tag v0.1.0");
    open_submenu(h, "Push to");
}

#[test]
fn graph_label_menu_tag_push() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "label-menu-tag-push");
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new()), ("backup".into(), Vec::new())];
    let mut h = harness(st, true);
    h.run();
    open_tag_submenu(&mut h);
    h.check_snapshot("graph_label_menu_tag_remotes");
    for item in ["origin", "backup", "All remotes"] {
        assert!(!submenu_item(&h, "Push to", item).is_disabled(), "{item}");
    }
    let tag = "v0.1.0".to_owned();
    let sent = |h: &super::snapshot_tests_worktrees::H, want: TagMsg| h.state().1.iter().any(|m| matches!(m, Msg::Tag(t) if *t == want));
    submenu_item(&h, "Push to", "backup").click();
    h.run();
    assert!(sent(&h, TagMsg::Push { tag: tag.clone(), remotes: vec!["backup".into()] }), "one remote");

    open_tag_submenu(&mut h);
    submenu_item(&h, "Push to", "All remotes").click();
    h.run();
    assert!(sent(&h, TagMsg::Push { tag: tag.clone(), remotes: vec!["origin".into(), "backup".into()] }), "every remote");

    open_tag_submenu(&mut h);
    h.get_by_label("Checkout (detached)").click();
    h.run();
    assert!(sent(&h, TagMsg::Checkout { tag }), "the tag checkout");

    // While a job runs, the push items are disabled; the spinner keeps repainting.
    running(&mut h.state_mut().0);
    h.run_steps(2);
    click_steps(&mut h, pos2(CHIP_X, row_y(MAIN_ROW)), PointerButton::Secondary);
    open_submenu(&mut h, "Tag v0.1.0");
    open_submenu(&mut h, "Push to");
    assert!(submenu_item(&h, "Push to", "origin").is_disabled(), "a job is running");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_menu_merges_the_branch_not_a_tag() {
    // A tag named like the branch, on main's first commit: git's short-name lookup would take it.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "label-menu-not-a-tag");
    let repo = git2::Repository::open(&dir).unwrap();
    let root = st.snap.as_ref().unwrap().commits.iter().find(|c| c.parents.is_empty()).unwrap().oid;
    repo.tag_lightweight("search", repo.find_commit(root).unwrap().as_object(), false).unwrap();
    reload(&mut st, &dir, &LoadOptions::all(500));
    let tip = repo.find_reference("refs/heads/search").unwrap().peel_to_commit().unwrap().id();
    let theirs = BranchRef::Local("search".into());
    let names = st.snap.as_ref().unwrap().merge_names.clone();
    assert!(names.is_shadowed(&theirs), "the load sees the tag shadow the branch");
    let mut h = harness(st, true);
    h.run();
    let sent_cmds = |h: &super::snapshot_tests_worktrees::H, want: &[String]| {
        h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Git { cmds, .. }) if cmds == &[want.to_vec()]))
    };
    for (item, want) in [("Merge into main", merge_cmd(&theirs, &names)), ("Rebase main onto this branch", rebase_cmd(&theirs))] {
        click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), PointerButton::Secondary);
        h.state_mut().1.clear();
        h.get_by_label(item).click();
        h.run();
        assert!(sent_cmds(&h, &want), "{item} sends {want:?}");
    }
    click(&mut h, pos2(CHIP_X, row_y(SEARCH_ROW)), PointerButton::Secondary);
    h.state_mut().1.clear();
    h.get_by_label("Preview merge").click();
    h.run();
    let want = PreviewOp::Merge { theirs, oid: tip };
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpPreview(OpPreviewMsg::Open(op)) if *op == want)), "the preview names the branch");
    drop(h);
    remove_fixture(&dir);
}
