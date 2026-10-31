//! The commit panel collapsed to a rail beside a tool view (blame, folder history), what its
//! entries send, the full panel once expanded, and the edge strips a hidden left or details panel
//! leaves (hovered, with its tooltip, and clicked). Shares the fixture repository and renderer
//! with `snapshot_tests`.

use crate::keymap::BindingId;
use crate::keymap::KeyAction;
use crate::msg::{LayoutMsg, Msg};
use crate::state::AppState;
use crate::state::layout::{Panel, ToolView};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::harness;
use crate::ui::snapshot_tests::snapshot_tests_blame::with_blame;
use crate::ui::snapshot_tests::snapshot_tests_history::with_folder;
use crate::ui::snapshot_tests::{reload, remove_fixture, with_repo};
use eframe::egui::{self, containers::panel::PanelState};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The blame fixture with an untracked file beside the edited README: two changed files.
fn blame_dirty(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir, _) = with_blame(theme, name);
    std::fs::write(dir.join("notes.txt"), "notes\n").unwrap();
    reload(&mut st, &dir, &LoadOptions::all(500));
    (st, dir)
}

/// Whether the egui panel `id` was drawn this frame (its stored rect is this frame's).
fn drawn(h: &H, id: &str) -> Option<egui::Rect> {
    PanelState::load(&h.ctx, egui::Id::new(id)).map(|p| p.rect)
}

#[test]
fn layout_details_rail_blame() {
    let (st, dir) = blame_dirty(ThemeChoice::Dark, "rail-blame");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("layout_details_rail_blame");
    assert!(drawn(&h, "detail").is_none(), "no full details panel");
    let rail = drawn(&h, "detail-rail").expect("the rail");
    assert_eq!(rail.width(), 44.0);
    assert!(h.query_by_label("2 changed files").is_some(), "the change count");
    h.get_by_label("Expand the commit panel").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Layout(LayoutMsg::ExpandDetails))));
    h.get_by_label("2 changed files").click();
    h.run();
    assert_eq!(h.state().1.iter().filter(|m| matches!(m, Msg::Layout(LayoutMsg::ExpandDetails))).count(), 2, "the count expands too");
    h.get_by_label("Go to the commit message").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Shortcut(KeyAction::FocusMessage))));
    // Nothing above was applied (the harness only collects messages): still a rail.
    assert!(h.state().0.settings.show_details_panel);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn layout_details_rail_folder_history_light() {
    let (st, dir) = with_folder("rail-folder", ThemeChoice::Light);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("layout_details_rail_folder_history_light");
    assert!(drawn(&h, "detail").is_none());
    // The fixture's edited README.
    assert!(h.query_by_label("1 changed file").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn layout_details_rail_expanded() {
    let (mut st, dir) = blame_dirty(ThemeChoice::Dark, "rail-expanded");
    st.view.details_expanded_for = Some(ToolView::Blame);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("layout_details_rail_expanded");
    assert!(drawn(&h, "detail").is_some(), "the full panel is back");
    assert!(h.query_by_label("Expand the commit panel").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn layout_details_hidden_in_blame() {
    let (mut st, dir) = blame_dirty(ThemeChoice::Dark, "rail-hidden-blame");
    st.settings.show_details_panel = false;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("layout_details_hidden_in_blame");
    assert!(h.query_by_label("Expand the commit panel").is_none(), "no rail");
    h.get_by_label("Show commit details panel").click();
    h.run();
    let want = LayoutMsg::SetPanel { panel: Panel::Details, shown: true };
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Layout(l) if *l == want)));
    drop(h);
    remove_fixture(&dir);
}

/// Whether the harness collected `SetPanel { panel, shown: true }`.
fn shows(h: &H, panel: Panel) -> bool {
    let want = LayoutMsg::SetPanel { panel, shown: true };
    h.state().1.iter().any(|m| matches!(m, Msg::Layout(l) if *l == want))
}

#[test]
fn layout_left_edge_hover() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rail-left-edge");
    st.settings.show_left_panel = false;
    let mut h = harness(st, true);
    h.run();
    assert_eq!(drawn(&h, "refs-edge").map(|r| r.width()), Some(16.0), "the strip");
    h.get_by_label("Show left panel").hover();
    // Past the tooltip's delay.
    for _ in 0..4 {
        h.run();
    }
    let st = &h.state().0;
    let tip = st.keymap.tip(st.settings.lang().tr("layout.edge.left.tip"), BindingId::ToggleLeftPanel);
    assert!(h.query_by_label(&tip).is_some(), "the tooltip names the key: {tip}");
    h.check_snapshot("layout_left_edge_hover");
    h.get_by_label("Show left panel").click();
    h.run();
    assert!(shows(&h, Panel::Left));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_edge_click_on_the_graph() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rail-details-edge");
    st.settings.show_details_panel = false;
    let mut h = harness(st, true);
    h.run();
    assert_eq!(drawn(&h, "detail-edge").map(|r| r.width()), Some(16.0), "the strip");
    assert!(drawn(&h, "detail").is_none() && drawn(&h, "detail-rail").is_none());
    h.get_by_label("Show commit details panel").click();
    h.run();
    assert!(shows(&h, Panel::Details));
    drop(h);
    remove_fixture(&dir);
}
