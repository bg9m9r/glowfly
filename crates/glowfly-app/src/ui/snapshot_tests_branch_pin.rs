//! Pinned branch colours and person colours: the Color submenu of the local and remote branch
//! menus (main's disabled), a pinned branch in the graph, the colour-blind safe preset on dark
//! and light palettes, and the initials of two people in the graph and the commit details.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{git, harness, harness_palette, reload, remove_fixture};
use super::snapshot_tests_branch_colour::with_branches;
use super::snapshot_tests_select::click;
use super::theme::{self, branch_colours::initials_tones, color};
use crate::msg::{GraphMsg, Msg};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::branch_colour::Swatch;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::{BranchPalette, ThemeChoice};
use std::path::Path;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const STANDARD: [&str; 14] =
    ["Rose", "Lime", "Violet", "Orange", "Cyan", "Magenta", "Olive", "Azure", "Red", "Teal", "Orchid", "Gold", "Sky", "Indigo"];

/// Right-clicks the left panel's row named `label`: the LOCAL one (`local`) or the REMOTE one.
fn open_row_menu(h: &mut H, label: &str, local: bool) {
    let rows = h
        .get_all_by_label(label)
        .filter_map(|n| n.bounding_box())
        // Below the toolbar (whose Branch picker names main), left of the graph.
        .filter(|r| r.y0 > 100.0 && r.x0 < 120.0);
    let rect = if local { rows.min_by(|a, b| a.y0.total_cmp(&b.y0)) } else { rows.max_by(|a, b| a.y0.total_cmp(&b.y0)) };
    let rect = rect.expect("the sidebar row");
    click(h, pos2(((rect.x0 + rect.x1) / 2.0) as f32, ((rect.y0 + rect.y1) / 2.0) as f32), PointerButton::Secondary);
}

fn pinned(h: &H) -> Option<(String, Option<u8>)> {
    h.state().1.iter().rev().find_map(|m| match m {
        Msg::Graph(GraphMsg::PinColour { key, slot }) => Some((key.clone(), *slot)),
        _ => None,
    })
}

/// Pins `key` to `slot` in the settings and recolours the snapshot as `update` would.
fn pin(st: &mut AppState, key: &str, slot: u8) {
    let snap = st.snap.as_mut().unwrap();
    st.settings.set_branch_pin(&snap.real_workdir, key, Some(slot));
    snap.set_colouring(st.settings.colouring_for(&snap.real_workdir));
}

#[test]
fn branch_colour_submenu() {
    let (st, dir) = with_branches(ThemeChoice::Dark, "colour-submenu");
    let mut h = harness(st, true);
    h.run();
    open_row_menu(&mut h, "topic", true);
    open_submenu(&mut h, "Color");
    submenu_item(&h, "Color", "✔ Automatic");
    for hue in STANDARD {
        submenu_item(&h, "Color", hue);
    }
    h.check_snapshot("branch_colour_submenu");
    submenu_item(&h, "Color", "Teal").click();
    h.run();
    assert_eq!(pinned(&h), Some(("topic".into(), Some(9))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_colour_main_disabled() {
    let (st, dir) = with_branches(ThemeChoice::Dark, "colour-main");
    let mut h = harness(st, true);
    h.run();
    // The checked-out branch's row carries the check glyph.
    open_row_menu(&mut h, "✔  main", true);
    assert!(h.get_by_label("Color").is_disabled(), "main keeps the accent");
    h.check_snapshot("branch_colour_main_disabled");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_branch_colour_submenu() {
    let (mut st, dir) = with_branches(ThemeChoice::Dark, "colour-remote");
    pin(&mut st, "topic", 3);
    let mut h = harness(st, true);
    h.run();
    open_row_menu(&mut h, "topic", false);
    assert!(h.query_by_label("origin/topic").is_some(), "the remote branch's menu");
    open_submenu(&mut h, "Color");
    submenu_item(&h, "Color", "Automatic");
    submenu_item(&h, "Color", "✔ Orange");
    h.check_snapshot("remote_branch_colour_submenu");
    submenu_item(&h, "Color", "Automatic").click();
    h.run();
    assert_eq!(pinned(&h), Some(("topic".into(), None)), "the key leaves the remote out");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_colour_pinned_graph() {
    let (mut st, dir) = with_branches(ThemeChoice::Dark, "colour-pinned");
    // Teal: both remote and local copies take it.
    pin(&mut st, "topic", 9);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.branch_swatch("topic"), Swatch::Slot(9));
    assert_eq!(snap.ref_swatch("refs/remotes/origin/topic"), Some(Swatch::Slot(9)));
    assert_ne!(snap.branch_swatch("fix-1"), Swatch::Slot(9));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("branch_colour_pinned_graph");
    drop(h);
    remove_fixture(&dir);
}

fn colour_blind(theme: ThemeChoice, name: &str) {
    let (mut st, dir) = with_branches(theme.clone(), name);
    st.settings.branch_palette = BranchPalette::ColourBlind;
    let snap = st.snap.as_mut().unwrap();
    snap.set_colouring(st.settings.colouring_for(&snap.real_workdir));
    let base = if theme == ThemeChoice::Dark { theme::GARDEN_DARK } else { theme::GARDEN_LIGHT };
    let mut h = harness_palette(st, theme::applied(base, BranchPalette::ColourBlind));
    h.run();
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_colours_colour_blind_dark() {
    colour_blind(ThemeChoice::Dark, "branch_colours_colour_blind_dark");
}

#[test]
fn branch_colours_colour_blind_light() {
    colour_blind(ThemeChoice::Light, "branch_colours_colour_blind_light");
}

/// `with_branches` with a commit by Grace Hopper on topic.
fn with_grace(theme: ThemeChoice, name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_branches(theme, name);
    let d = "2026-01-13T10:00:00+00:00";
    git(&dir, &["checkout", "-q", "topic"], d);
    std::fs::write(dir.join("grace.txt"), "grace").unwrap();
    git(&dir, &["add", "grace.txt"], d);
    git(&dir, &["commit", "-q", "--author", "Grace Hopper <grace@example.com>", "-m", "Compile the topic"], d);
    git(&dir, &["checkout", "-q", "main"], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    (st, dir)
}

fn tip(dir: &Path, branch: &str) -> git2::Oid {
    git2::Repository::open(dir).unwrap().refname_to_id(&format!("refs/heads/{branch}")).unwrap()
}

fn person_colours(theme: ThemeChoice, name: &str, dark: bool) {
    let (st, dir) = with_grace(theme, name);
    color::set_palette(if dark { &theme::GARDEN_DARK } else { &theme::GARDEN_LIGHT });
    let ada = initials_tones("ada@example.com", "Ada Lovelace");
    let grace = initials_tones("grace@example.com", "Grace Hopper");
    assert_ne!(ada.0, grace.0, "two people, two fills");
    let mut h = harness(st, dark);
    h.run();
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn person_colours_graph_dark() {
    person_colours(ThemeChoice::Dark, "person_colours_graph_dark", true);
}

#[test]
fn person_colours_graph_light() {
    person_colours(ThemeChoice::Light, "person_colours_graph_light", false);
}

#[test]
fn details_person_colour() {
    let (mut st, dir) = with_grace(ThemeChoice::Dark, "details_person_colour");
    let oid = tip(&dir, "topic");
    st.selection = Selection::Commit(oid);
    st.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    assert_eq!(st.detail.as_ref().unwrap().email, "grace@example.com");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("details_person_colour");
    drop(h);
    remove_fixture(&dir);
}
