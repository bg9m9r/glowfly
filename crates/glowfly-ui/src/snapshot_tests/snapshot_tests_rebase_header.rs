//! The rebase panel's header and list ends at a narrow window: the subtitle wraps instead of
//! being cut, the faint order line says which end is the tip, and a faint "Tip of main" row
//! with the branch's chip follows the last step (none while the list is empty).

use super::snapshot_tests::harness_sized;
use super::snapshot_tests::remove_fixture;
use super::snapshot_tests_rebase::with_panel;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::RebaseAction;
use glowfly_core::settings::ThemeChoice;

#[test]
fn rebase_header_narrow_light() {
    use RebaseAction::*;
    let (st, dir) = with_panel(ThemeChoice::Light, "rebase-header-narrow", &[Pick, Reword, Pick, Squash, Pick, Pick]);
    let subtitle = st.session.rebase.view.as_ref().unwrap().subtitle(Lang::En);
    let mut h = harness_sized(st, false, egui::vec2(1100.0, 820.0));
    h.run();
    h.check_snapshot("rebase_header_narrow_light");
    let line = h.get_by_label(&subtitle).bounding_box().expect("the subtitle is laid out");
    let more = h.get_by_label("More rebase actions").bounding_box().expect("the ⋯ button");
    assert!(line.x1 <= more.x0, "the subtitle stays left of the buttons: {line:?} {more:?}");
    assert!(line.y1 - line.y0 > 24.0, "it wraps onto a second line rather than being cut: {line:?}");
    let order = "Oldest first: commits are applied top to bottom, and the last becomes the tip of main.";
    assert!(h.query_by_label(order).is_some(), "the order line");
    let tip = h.get_by_label("Tip of main").bounding_box().expect("the tip row");
    let last = h.get_by_label("Search results page").bounding_box().expect("the last step");
    assert!(tip.y0 >= last.y1 && tip.y1 - tip.y0 <= 24.5, "a 24 px row after the last step: {tip:?} {last:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn an_empty_list_has_no_tip_row() {
    use RebaseAction::*;
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-header-empty", &[Pick, Pick]);
    st.session.rebase.view.as_mut().unwrap().items.clear();
    let mut h = harness_sized(st, true, egui::vec2(1100.0, 820.0));
    h.run();
    assert!(h.query_by_label("Tip of main").is_none(), "nothing to end on");
    drop(h);
    remove_fixture(&dir);
}
