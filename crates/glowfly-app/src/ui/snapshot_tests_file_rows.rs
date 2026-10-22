//! File rows as one click target: a click on a row's empty area or status letter opens the file
//! (WIP, commit and folder history rows), a click on its Stage button only stages, a right-click
//! on its empty area opens its menu, and the selected and hovered rows' look (dark and light).
//! Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use super::snapshot_tests_history::with_folder;
use super::snapshot_tests_line_counts::with_wip;
use super::snapshot_tests_select::click;
use crate::msg::{HistoryMsg, Msg, RepoMsg, StageMsg};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The row named `name`'s left and right edges and its centre's y.
fn row(h: &H, name: &str) -> (f32, f32, f32) {
    let b = h.get_by_role_and_label(Role::Button, name).bounding_box().expect("the row has bounds");
    (b.x0 as f32, b.x1 as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// The messages a `button` click at `at` sends.
fn click_msgs(h: &mut H, at: Pos2, button: PointerButton) -> Vec<Msg> {
    h.state_mut().1.clear();
    click(h, at, button);
    std::mem::take(&mut h.state_mut().1)
}

fn opens(msgs: &[Msg], target: &DiffTarget) -> bool {
    msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::OpenDiff(t)) if t == target))
}

#[test]
fn wip_row_click_on_empty_area_opens_the_diff() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "file-rows-wip");
    let mut h = harness(st, true);
    h.run();
    let readme = DiffTarget::Unstaged("README.md".into());
    let (x0, x1, y) = row(&h, "README.md");
    let msgs = click_msgs(&mut h, pos2(x1 - 12.0, y), PointerButton::Primary);
    assert!(opens(&msgs, &readme), "a click right of the path opens the diff");
    let (st, msgs) = h.state_mut();
    for m in std::mem::take(msgs) {
        crate::update::update(st, m, Instant::now());
    }
    h.run_ok();
    let msgs = click_msgs(&mut h, pos2(x0 + 4.0, y), PointerButton::Primary);
    assert!(opens(&msgs, &readme), "a click on the status letter opens it too");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_row_button_click_only_stages() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "file-rows-stage");
    let mut h = harness(st, true);
    h.run();
    let (x0, x1, y) = row(&h, "README.md");
    // The row's Stage shows while the pointer is over the row.
    h.input_mut().events.push(eframe::egui::Event::PointerMoved(pos2((x0 + x1) / 2.0, y)));
    h.run_ok();
    let stage = h
        .get_all_by_role_and_label(Role::Button, "Stage")
        .filter_map(|n| n.bounding_box())
        .find(|b| (b.x0 as f32) >= x0 && (b.x1 as f32) <= x1 && (b.y0 as f32) <= y && (b.y1 as f32) >= y)
        .expect("the row's Stage button");
    let centre = pos2(((stage.x0 + stage.x1) / 2.0) as f32, ((stage.y0 + stage.y1) / 2.0) as f32);
    let msgs = click_msgs(&mut h, centre, PointerButton::Primary);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Stage(StageMsg::StageFile(p)) if p == "README.md")));
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::OpenDiff(_)))), "the button does not open the diff");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_row_right_click_on_empty_area_opens_its_menu() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "file-rows-menu");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Copy path").is_none());
    let (_, x1, y) = row(&h, "README.md");
    click_msgs(&mut h, pos2(x1 - 12.0, y), PointerButton::Secondary);
    assert!(h.query_by_label("Copy path").is_some(), "the row's menu is open");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_row_click_on_empty_area_opens_the_diff() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "file-rows-commit");
    std::fs::write(dir.join("notes.txt"), "notes\n").unwrap();
    git(&dir, &["add", "notes.txt"], "2026-01-20T10:00:00+00:00");
    git(&dir, &["commit", "-q", "-m", "Add notes"], "2026-01-20T10:00:00+00:00");
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.selection = Selection::Commit(oid);
    st.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    let mut h = harness(st, true);
    h.run();
    let (_, x1, y) = row(&h, "notes.txt");
    let msgs = click_msgs(&mut h, pos2(x1 - 12.0, y), PointerButton::Primary);
    assert!(opens(&msgs, &DiffTarget::Commit(oid, "notes.txt".into())));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn folder_history_file_row_click_on_empty_area_selects_it() {
    let (st, dir) = with_folder("file-rows-folder", ThemeChoice::Dark);
    let mut h = harness(st, true);
    h.run();
    let (_, x1, y) = row(&h, "lexer.rs");
    let msgs = click_msgs(&mut h, pos2(x1 - 12.0, y), PointerButton::Primary);
    assert!(msgs.iter().any(|m| matches!(m, Msg::History(HistoryMsg::SelectHistoryFile(0)))));
    drop(h);
    remove_fixture(&dir);
}

/// The working tree with line counts, app.txt's unstaged diff open and the pointer over the
/// Staged Files row of search.txt.
fn selected_and_hovered(theme: ThemeChoice, dark: bool, name: &str) {
    let (mut st, dir) = with_wip(theme, "file-rows-look");
    let target = DiffTarget::Unstaged("app.txt".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    st.diff = Some((target, diff.into()));
    let mut h = harness(st, dark);
    h.run();
    let (_, x1, y) = row(&h, "search.txt");
    h.input_mut().events.push(Event::PointerMoved(pos2(x1 - 40.0, y)));
    for _ in 0..4 {
        h.step();
    }
    h.check_snapshot(name);
    drop(h);
    if !dark {
        remove_fixture(&dir);
    }
}

#[test]
fn file_rows_selected_and_hovered() {
    selected_and_hovered(ThemeChoice::Dark, true, "file_rows_selected_and_hovered");
    selected_and_hovered(ThemeChoice::Light, false, "file_rows_selected_and_hovered_light");
}
