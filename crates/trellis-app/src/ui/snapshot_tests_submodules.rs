//! The SUBMODULES section: one row per state with its marker and word, the left-panel filter,
//! and the row menu with its disabled items. The submodules are injected into the shared
//! fixture's snapshot, which keeps the images deterministic and needs no clones.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_select::click;
use crate::msg::Msg;
use crate::state::AppState;
use crate::submodule::SubmoduleMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::repo::{SubmoduleInfo, SubmoduleState};
use trellis_core::settings::ThemeChoice;

fn oid(c: char) -> Oid {
    Oid::from_str(&c.to_string().repeat(40)).unwrap()
}

fn info(path: &str, state: SubmoduleState, url: Option<&str>) -> SubmoduleInfo {
    let checked_out = match state {
        SubmoduleState::Uninitialized => None,
        SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync => Some(oid('b')),
        _ => Some(oid('a')),
    };
    SubmoduleInfo {
        name: path.into(),
        path: path.into(),
        url: url.map(str::to_owned),
        recorded: oid('a'),
        checked_out,
        state,
        left_behind: Vec::new(),
    }
}

/// The shared fixture with one submodule per state.
fn with_submodules(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.snap.as_mut().unwrap().submodules = vec![
        info("libs/core", SubmoduleState::InSync, Some("../core.git")),
        info("libs/ui", SubmoduleState::OutOfSync, Some("https://example.com/ui.git")),
        info("tools/gen", SubmoduleState::Uninitialized, None),
        info("vendor/json", SubmoduleState::Dirty, Some("../json.git")),
        info("vendor/zlib", SubmoduleState::DirtyOutOfSync, Some("../zlib.git")),
    ];
    (st, dir)
}

/// Right-clicks the row labelled `path`.
fn open_menu(h: &mut Harness<'_, (AppState, Vec<Msg>)>, path: &str) {
    let rect = h.get_by_label(path).bounding_box().expect("the row");
    click(h, pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32), PointerButton::Secondary);
}

fn pushed(h: &Harness<'_, (AppState, Vec<Msg>)>, want: &dyn Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn submodules_section() {
    let (st, dir) = with_submodules(ThemeChoice::Dark, "submodules");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("libs/core").is_some());
    h.check_snapshot("submodules_section");
    drop(h);
    let (st, _) = with_submodules(ThemeChoice::Light, "submodules");
    render("submodules_section_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn submodules_filtered() {
    let (mut st, dir) = with_submodules(ThemeChoice::Dark, "submodules-filtered");
    st.view.ref_filter = "VENDOR".into();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("vendor/json").is_some() && h.query_by_label("vendor/zlib").is_some());
    assert!(h.query_by_label("libs/core").is_none());
    h.check_snapshot("submodules_filtered");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_menu() {
    let (st, dir) = with_submodules(ThemeChoice::Dark, "submodule-menu");
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "libs/ui");
    h.check_snapshot("submodule_menu");
    h.get_by_label("Update to recorded commit").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Update { path, confirmed: false }) if path == "libs/ui")));
    drop(h);

    let (st, _) = with_submodules(ThemeChoice::Dark, "submodule-menu");
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "libs/ui");
    h.get_by_label("Copy URL").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Copy(u) if u == "https://example.com/ui.git")));
    open_menu(&mut h, "libs/ui");
    h.get_by_label("Open in new tab").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Open(p)) if p == "libs/ui")));
    drop(h);

    // Not initialized and no URL: nothing applies.
    let (mut st, _) = with_submodules(ThemeChoice::Dark, "submodule-menu");
    st.snap.as_mut().unwrap().submodules.push(info("tools/lint", SubmoduleState::Uninitialized, Some("../lint.git")));
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "tools/gen");
    assert!(h.get_by_label("Open in new tab").is_disabled());
    assert!(h.get_by_label("Initialize").is_disabled());
    assert!(h.get_by_label("Copy URL").is_disabled());
    h.press_key(eframe::egui::Key::Escape);
    h.run();

    // Not initialized with a URL: Initialize is offered, opening still isn't.
    open_menu(&mut h, "tools/lint");
    assert!(!h.get_by_label("Initialize").is_disabled());
    assert!(h.get_by_label("Open in new tab").is_disabled());
    h.get_by_label("Initialize").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Update { path, confirmed: false }) if path == "tools/lint")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_remove_menu_item() {
    let (st, dir) = with_submodules(ThemeChoice::Dark, "submodule-remove-item");
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "libs/core");
    h.get_by_label("Remove submodule…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::RemoveDialog(p)) if p == "libs/core")));
    drop(h);

    // No .gitmodules entry, or unstaged changes to .gitmodules: git would refuse.
    let (mut st, _) = with_submodules(ThemeChoice::Dark, "submodule-remove-item");
    st.snap.as_mut().unwrap().unstaged.push(trellis_core::repo::FileEntry::new(".gitmodules", 'M'));
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "libs/core");
    assert!(h.get_by_label("Remove submodule…").is_disabled());
    h.press_key(eframe::egui::Key::Escape);
    h.run();
    drop(h);
    let (st, _) = with_submodules(ThemeChoice::Dark, "submodule-remove-item");
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "tools/gen");
    assert!(h.get_by_label("Remove submodule…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_header_menu() {
    let (st, dir) = with_submodules(ThemeChoice::Dark, "submodule-header-menu");
    let mut h = harness(st, true);
    h.run();
    // The header, 12 + 15 px above the first row's center.
    let rect = h.get_by_label("libs/core").bounding_box().expect("the row");
    let row = pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32);
    click(&mut h, pos2(row.x, row.y - 27.0), PointerButton::Secondary);
    h.check_snapshot("submodule_header_menu");
    h.get_by_label("Add submodule…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::AddDialog))));
    drop(h);
    remove_fixture(&dir);
}
