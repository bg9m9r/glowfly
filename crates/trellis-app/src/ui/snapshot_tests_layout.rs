//! Hidden panels and the status bar's zoom and Show toolbar buttons. Shares the fixture
//! repository and renderer with `snapshot_tests`.

use crate::msg::{LayoutMsg, Msg};
use crate::state::layout::Panel;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, harness_sized, remove_fixture, render, settings, with_repo};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::time::Instant;
use trellis_core::settings::ThemeChoice;

fn pushed(h: &Harness<'_, (AppState, Vec<Msg>)>, want: &LayoutMsg) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Layout(l) if l == want))
}

#[test]
fn layout_side_panels_hidden() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "layout-sides");
    st.settings.show_left_panel = false;
    st.settings.show_details_panel = false;
    render("layout_side_panels_hidden", st, true);
    remove_fixture(&dir);
}

#[test]
fn layout_toolbar_hidden() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "layout-toolbar");
    st.settings.show_toolbar = false;
    st.settings.zoom_percent = 125;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("layout_toolbar_hidden");
    h.get_by_label("Show toolbar").click();
    h.run();
    assert!(pushed(&h, &LayoutMsg::SetPanel { panel: Panel::Toolbar, shown: true }));
    h.get_by_label("Zoom 125%").click();
    h.run();
    assert!(pushed(&h, &LayoutMsg::ZoomReset));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn layout_all_hidden_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "layout-all");
    st.settings.show_left_panel = false;
    st.settings.show_details_panel = false;
    st.settings.show_toolbar = false;
    render("layout_all_hidden_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn layout_sheet_view_group() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "layout-sheet");
    st.modal = Some(Modal::Shortcuts { filter: "view".into() });
    render("layout_sheet_view_group", st, true);
    remove_fixture(&dir);
}

#[test]
fn layout_prefs_interface() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.settings.zoom_percent = 125;
    st.settings.show_toolbar = false;
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 1300.0));
    h.run();
    h.check_snapshot("layout_prefs_interface");
}

/// The reported case: a wide window with a commit selected and the details drawer once
/// dragged far too narrow. The drawer is drawn at its minimum instead, and with no saved
/// width at its default for the window.
#[test]
fn layout_wide_window_details_drawer() {
    use trellis_core::repo;
    use trellis_core::settings::SizedPanel;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "layout-wide");
    let snap = st.snap.as_ref().unwrap();
    let oid = snap.commits.iter().find(|c| c.summary == "Fix build on Windows").expect("the fix commit").oid;
    st.selection = crate::state::Selection::Commit(oid);
    st.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    let wide = egui::vec2(2000.0, 1070.0);
    let mut h = harness_sized(st, true, wide);
    h.run();
    h.check_snapshot("layout_wide_window_details_default");
    h.state_mut().0.settings.panel_sizes.set(SizedPanel::Details, 120.0);
    h.run();
    h.check_snapshot("layout_wide_window_details_too_small_saved");
    drop(h);
    remove_fixture(&dir);
}
