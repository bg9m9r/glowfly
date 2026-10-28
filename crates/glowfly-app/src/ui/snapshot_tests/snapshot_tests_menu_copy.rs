//! The copy items of the commit, multi-commit, tag, local branch and remote branch menus (a
//! Copy submenu for the commit's four, in the menu for the others' two): what each copies, and
//! the patch entry disabled for a merge. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use super::snapshot_tests_worktrees::{H, pushed};
use crate::deep_link::{DeepLinkMsg, LinkTarget};
use crate::msg::Msg;
use crate::state::selection::MultiSelection;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::settings::ThemeChoice;

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

fn row(st: &AppState, oid: Oid) -> usize {
    st.snap.as_ref().unwrap().row_of[&oid] + 1
}

fn copied(h: &H, text: &str) -> bool {
    pushed(h, &|m| matches!(m, Msg::Copy(t) if t == text))
}

fn linked(h: &H, want: LinkTarget) -> bool {
    pushed(h, &|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Copy(t)) if *t == want))
}

/// Clicks `label` in the Copy submenu of the menu `open` opens.
fn copy(h: &mut H, open: &dyn Fn(&mut H), label: &str) {
    open(h);
    open_submenu(h, "Copy");
    submenu_item(h, "Copy", label).click();
    h.run();
}

/// Clicks `label`, a copy item the menu `open` opens holds itself (two or fewer, no submenu).
fn flat(h: &mut H, open: &dyn Fn(&mut H), label: &str) {
    open(h);
    h.get_by_label(label).click();
    h.run();
}

/// The middle of the sidebar row labelled `label`: the topmost (LOCAL) or the lowest (REMOTE)
/// left-panel match, near its left end so an open menu doesn't cover it.
fn sidebar_row(h: &H, label: &str, lowest: bool) -> Pos2 {
    let rows = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).filter(|r| r.x0 < 250.0);
    let r = if lowest { rows.max_by(|a, b| a.y0.total_cmp(&b.y0)) } else { rows.min_by(|a, b| a.y0.total_cmp(&b.y0)) };
    let r = r.expect("the sidebar row");
    pos2(r.x0 as f32 + 40.0, ((r.y0 + r.y1) / 2.0) as f32)
}

#[test]
fn graph_commit_menu_copy_submenu() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "commit-menu-copy");
    let fix = commit(&st, "Fix build on Windows");
    let at = pos2(700.0, row_y(row(&st, fix)));
    let mut h = harness(st, true);
    h.run();
    let open = |h: &mut H| click(h, at, PointerButton::Secondary);
    open(&mut h);
    open_submenu(&mut h, "Copy");
    h.check_snapshot("graph_commit_menu_copy_submenu");
    submenu_item(&h, "Copy", "SHA").click();
    h.run();
    assert!(copied(&h, &fix.to_string()), "the full SHA");
    copy(&mut h, &open, "Message");
    assert!(copied(&h, "Fix build on Windows"));
    copy(&mut h, &open, "Link");
    assert!(linked(&h, LinkTarget::Commit(fix)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn multi_commit_menu_copy_patch_merge() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "multi-menu-copy-merge");
    let (merge, fix) = (commit(&st, "Merge branch 'login-page'"), commit(&st, "Fix build on Windows"));
    st.session.selection = MultiSelection::build([merge, fix], merge, st.snap.as_ref().unwrap());
    let Selection::Commits(m) = &st.session.selection else { panic!("not a multi-selection") };
    let why = m.export_blocked.expect("a merge has no patch of its own");
    let at = pos2(700.0, row_y(row(&st, merge)));
    let mut h = harness(st, true);
    h.run_steps(4);
    click(&mut h, at, PointerButton::Secondary);
    assert!(h.get_by_label("Copy patch").is_disabled(), "{why:?}");
    // The selection panel has a Copy SHAs button too; neither is blocked.
    assert!(h.get_all_by_label("Copy SHAs").all(|n| !n.is_disabled()));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn ref_menus_copy_names() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "ref-menus-copy");
    let search = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![("search".into(), search)])];
    st.view.sections.tags = true;
    let mut h = harness(st, true);
    h.run();

    // A tag: its name, and a link to it.
    let tag = |h: &mut H| {
        let at = sidebar_row(h, "v0.1.0", false);
        click(h, at, PointerButton::Secondary);
    };
    flat(&mut h, &tag, "Copy name");
    assert!(copied(&h, "v0.1.0"));

    // A local branch.
    let local = |h: &mut H| {
        let at = sidebar_row(h, "search", false);
        click(h, at, PointerButton::Secondary);
    };
    flat(&mut h, &local, "Copy name");
    assert!(copied(&h, "search"));
    flat(&mut h, &local, "Copy link");
    assert!(linked(&h, LinkTarget::LocalBranch("search".into())));

    // A remote branch copies its full name.
    let remote = |h: &mut H| {
        let at = sidebar_row(h, "search", true);
        click(h, at, PointerButton::Secondary);
    };
    flat(&mut h, &remote, "Copy name");
    assert!(copied(&h, "origin/search"));
    drop(h);
    remove_fixture(&dir);
}
