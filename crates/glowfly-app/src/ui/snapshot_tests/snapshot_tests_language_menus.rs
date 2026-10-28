//! German for the hunk buttons and the left-panel, graph-chip and commit menus, with their
//! confirmations: no English text from this item's areas, the asks in German.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_diff::with_diff;
use super::snapshot_tests_language_main::{H, assert_no_english, german, panels};
use super::snapshot_tests_revert::with_commit_diff;
use super::snapshot_tests_select::{click, row_y};
use crate::msg::Msg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{DiffLayout, ThemeChoice};

/// The last confirmation asked for: its title, body and button.
fn confirm_danger(h: &H) -> Option<(String, String, String)> {
    h.state().1.iter().rev().find_map(|m| match m {
        Msg::ConfirmDanger { title, body, button, .. } => Some((title.clone(), body.clone(), button.clone())),
        _ => None,
    })
}

#[test]
fn diff_hunks_german() {
    let de = Lang::De;
    let (mut st, dir) = with_diff(DiffLayout::Inline, "lang-main-hunks");
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(de.tr("diff.hunk.stage")).is_some(), "Stage hunk in German");
    assert!(h.query_by_label(de.tr("diff.hunk.discard")).is_some(), "Discard hunk in German");
    assert_no_english(&h, "a working-tree diff");
    h.check_snapshot("diff_hunks_german");
    h.get_by_label(de.tr("diff.hunk.discard")).click();
    h.run();
    let body = de.trf("diff.hunk.discard.body", &[("path", &"app.txt")]);
    let want = (de.tr("diff.hunk.discard.title").to_owned(), body, de.tr("common.discard.button").to_owned());
    assert_eq!(confirm_danger(&h), Some(want), "the confirmation is German");
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_commit_diff("lang-main-revert", ThemeChoice::Dark, DiffLayout::Inline, false);
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(de.tr("diff.hunk.revert")).is_some(), "Revert hunk in German");
    assert_no_english(&h, "a commit diff");
    drop(h);
    remove_fixture(&dir);
}

/// The middle of the left-panel row `name`: rows start at the panel's edge; the `nth` from the
/// top (LOCAL comes before REMOTE).
fn sidebar_row(h: &H, name: &str, nth: usize) -> Pos2 {
    let mut rows: Vec<_> = h.get_all_by_label(name).filter_map(|n| n.bounding_box()).filter(|b| b.x0 < 40.0).collect();
    rows.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    let r = rows.get(nth).unwrap_or_else(|| panic!("left-panel row {nth} named {name}: {rows:?}"));
    pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32)
}

/// Right-clicks the left-panel row `name` (the `nth` from the top).
fn right_click_row(h: &mut H, name: &str, nth: usize) {
    let at = sidebar_row(h, name, nth);
    click(h, at, PointerButton::Secondary);
}

/// Closes the open menu: Esc closes one level, an open submenu first.
fn close_menu(h: &mut H) {
    for _ in 0..3 {
        if !h.ctx.is_context_menu_open() {
            return;
        }
        h.press_key(eframe::egui::Key::Escape);
        h.run_ok();
    }
}

/// Where the first chip of a graph row sits, and the fixture's rows (the WIP row is 0).
const CHIP_X: f32 = 290.0;
const SEARCH_ROW: usize = 1;
const MAIN_ROW: usize = 2;

#[test]
fn menus_german() {
    let de = Lang::De;
    let (st, dir) = panels("lang-main-menus");
    let mut h = harness(st, true);
    h.run();

    // LOCAL search.
    right_click_row(&mut h, "search", 0);
    let merge = de.trf("branch.menu.merge_into", &[("branch", &"main")]);
    assert!(h.query_by_label(&merge).is_some(), "the branch menu is German");
    assert_no_english(&h, "the local branch menu");
    h.check_snapshot("branch_menu_german");
    open_submenu(&mut h, de.tr("branch.menu.push_to"));
    let _ = submenu_item(&h, de.tr("branch.menu.push_to"), "backup");
    assert_no_english(&h, "the Push to submenu");
    assert!(h.query_by_label(de.tr("refs.copy.link_item")).is_some(), "the copy items, in the menu");
    close_menu(&mut h);

    // REMOTE origin/search.
    right_click_row(&mut h, "search", 1);
    assert!(h.query_by_label(de.tr("remote_branch.menu.delete")).is_some(), "the remote branch menu is German");
    assert_no_english(&h, "the remote branch menu");
    close_menu(&mut h);

    // TAGS v0.1.0.
    right_click_row(&mut h, "v0.1.0", 0);
    assert!(h.query_by_label(de.tr("tag.menu.delete_locally")).is_some(), "the tag menu is German");
    assert_no_english(&h, "the tag menu");
    open_submenu(&mut h, de.tr("tag.menu.delete_from_remote"));
    assert!(h.query_by_label(de.tr("tag.menu.all_remotes_ask")).is_some());
    assert_no_english(&h, "the tag's Delete from remote submenu");
    close_menu(&mut h);

    // A graph row away from its chips: the commit menu, with its Reset submenu.
    click(&mut h, pos2(700.0, row_y(SEARCH_ROW)), PointerButton::Secondary);
    assert!(h.query_by_label(de.tr("commits.menu.compare_workdir")).is_some(), "the commit menu is German");
    assert_no_english(&h, "the commit menu");
    h.check_snapshot("commit_menu_german");
    let reset = de.trf("commits.menu.reset.title", &[("branch", &"main")]);
    open_submenu(&mut h, &reset);
    assert!(h.query_by_label(de.tr("commits.menu.reset.hard")).is_some());
    assert_no_english(&h, "the Reset submenu");
    close_menu(&mut h);

    // main's tip carries the main branch and the v0.1.0 tag: a submenu each.
    click(&mut h, pos2(CHIP_X, row_y(MAIN_ROW)), PointerButton::Secondary);
    let branch = de.trf("refs.menu.branch", &[("name", &"main")]);
    let tag = de.trf("refs.menu.tag", &[("name", &"v0.1.0")]);
    assert!(h.query_by_label(&branch).is_some() && h.query_by_label(&tag).is_some(), "the chip submenus");
    open_submenu(&mut h, &tag);
    assert!(h.query_by_label(de.tr("commits.menu.checkout_detached")).is_some(), "the tag's menu");
    assert_no_english(&h, "the chips' submenus");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_confirmations_german() {
    let de = Lang::De;
    let (st, dir) = panels("lang-main-confirm");
    let mut h = harness(st, true);
    h.run();
    right_click_row(&mut h, "search", 0);
    h.get_by_label(de.tr("branch.menu.delete")).click();
    h.run();
    let dialog = |m: &Msg| matches!(m, Msg::Branch(crate::msg::BranchMsg::DeleteDialog(b)) if b == "search");
    assert!(h.state().1.iter().any(dialog), "the branch's Delete opens its dialog");

    right_click_row(&mut h, "v0.1.0", 0);
    h.get_by_label(de.tr("tag.menu.delete_locally")).click();
    h.run();
    let body = de.trf("tag.delete.body", &[("tag", &"v0.1.0")]);
    let want = (de.tr("tag.delete.title").to_owned(), body, de.tr("common.delete.button").to_owned());
    assert_eq!(confirm_danger(&h), Some(want), "the tag's Delete locally asks in German");
    drop(h);
    remove_fixture(&dir);
}
