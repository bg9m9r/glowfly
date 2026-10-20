//! The repository tabs' look with the system frame: the lifted active tab, the separators
//! between quiet tabs, a cut name with its whole name first in the tooltip, and the hover
//! shape, and the active tab's line in its checked-out branch's colour (grouped and not). Then
//! the redrawn Branch icon and the menu icon at their sizes, in Dark and Light.

use crate::state::{AppState, Tab};
use crate::ui::menu_testing::texts_containing;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::ui::tabs_tests::{pointer, tab_centre};
use crate::ui::theme::{self, color};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, pos2, vec2};
use egui_kittest::Harness;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;

/// A name of 60 characters, too long for a tab.
const LONG: &str = "a-repository-whose-folder-name-runs-on-past-what-a-tab-holds";

/// The fixture with five tabs, the fourth with a 60-character name.
fn five_tabs(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.settings.reduce_motion = true;
    let base = trellis_core::paths::temp_dir().join("trellis-tabs-style");
    st.tabs.push(Tab::unloaded(base.join("docs")));
    st.tabs.push(Tab::unloaded(base.join(LONG)));
    st.tabs.push(Tab::unloaded(base.join("web")));
    (st, dir)
}

#[test]
fn tabs_restyled() {
    assert_eq!(LONG.chars().count(), 60);
    let (st, dir) = five_tabs("tabs-restyled");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tabs_restyled");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tabs_hover() {
    let (st, dir) = five_tabs("tabs-hover");
    let mut h = harness(st, true);
    h.run();
    // The fixture's second tab, whose path (in its tooltip) is the same on every machine.
    let at = tab_centre(&h, 1) + vec2(-30.0, 0.0);
    pointer(&mut h, at, None);
    h.run();
    h.check_snapshot("tabs_hover");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_cut_tab_shows_its_whole_name_first() {
    let (st, dir) = five_tabs("tabs-cut-tip");
    let mut h = harness(st, true);
    h.run();
    let at = tab_centre(&h, 3) + vec2(-40.0, 0.0);
    pointer(&mut h, at, None);
    for _ in 0..4 {
        h.step();
    }
    h.run();
    let tip = texts_containing(&h, LONG).into_iter().find(|t| t.contains('\n')).expect("the tooltip");
    assert_eq!(tip.lines().next(), Some(LONG), "{tip:?}");
    assert!(tip.lines().nth(1).is_some_and(|l| l.ends_with(LONG)), "the path follows: {tip:?}");
    drop(h);
    remove_fixture(&dir);
}

/// `st`'s repository (`dir`) with topic checked out, from the branch-colour fixture's branches.
fn on_topic(st: &mut AppState, dir: &std::path::Path) {
    crate::ui::snapshot_tests::git(dir, &["checkout", "-q", "-b", "topic"], "2026-01-11T10:00:00+00:00");
    crate::ui::snapshot_tests::reload(st, dir, &trellis_core::repo::LoadOptions::all(500));
}

#[test]
fn active_tab_branch_line() {
    let (mut st, dir) = five_tabs("tabs-branch-line");
    on_topic(&mut st, &dir);
    assert!(st.snap.as_ref().unwrap().head_swatch().is_some_and(|s| s != trellis_core::branch_colour::Swatch::Main));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("active_tab_branch_line");
    drop(h);
    let (mut st, dir2) = crate::ui::tabs_tests::grouped_tabs("tabs-branch-line-grouped");
    on_topic(&mut st, &dir2);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("active_tab_branch_line_grouped");
    drop(h);
    remove_fixture(&dir);
    remove_fixture(&dir2);
}

#[test]
fn icon_branch() {
    let mut h = Harness::builder().with_size(vec2(360.0, 160.0)).build_ui(|ui| {
        let full = ui.max_rect();
        for (row, p) in [theme::DARK, theme::LIGHT].into_iter().enumerate() {
            color::set_palette(&p);
            theme::apply_style(ui.ctx(), &p);
            let band = egui::Rect::from_min_size(pos2(full.left(), full.top() + 80.0 * row as f32), vec2(full.width(), 80.0));
            ui.painter().rect_filled(band, 0.0, p.toolbar);
            let y = band.top() + 40.0;
            for (i, size) in [16.0, 20.0, 24.0].into_iter().enumerate() {
                widgets::paint_icon_sized(ui.painter(), pos2(band.left() + 24.0 + 36.0 * i as f32, y), Icon::Branch, p.text, size);
            }
            widgets::paint_icon_sized(ui.painter(), pos2(band.left() + 132.0, y), Icon::Branch, p.accent, 14.0);
            widgets::paint_icon(ui.painter(), pos2(band.left() + 166.0, y), Icon::Menu, p.text);
            let slot = egui::Rect::from_min_size(pos2(band.left() + 200.0, band.top() + 16.0), vec2(150.0, widgets::TOOL_H));
            ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
                ui.horizontal(|ui| widgets::tool_button(ui, Icon::Branch, "Branch", "Create a branch", true, true));
            });
        }
    });
    h.run();
    h.check_snapshot("icon_branch");
}
