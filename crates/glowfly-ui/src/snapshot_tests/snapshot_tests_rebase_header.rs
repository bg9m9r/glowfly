//! The rebase panel's header and list ends at a narrow window: the subtitle wraps instead of
//! being cut, the faint order line says which end is the tip, and a faint "Tip of main" row
//! with the branch's chip follows the last step (none while the list is empty). Also the other
//! rows moving apart round a dragged one: over `motion::SHIFT`, or at once with Reduce motion.

use super::snapshot_tests::remove_fixture;
use super::snapshot_tests::{harness_sized, harness_stepped};
use super::snapshot_tests_rebase::{pointer, with_panel};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Shape, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::RebaseAction;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;

#[test]
fn rebase_header_narrow_light() {
    use RebaseAction::*;
    let (mut st, dir) = with_panel(ThemeChoice::Light, "rebase-header-narrow", &[Pick, Reword, Pick, Squash, Pick, Pick]);
    // Onto a branch with a long name, so the subtitle needs a second line beside the buttons.
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let plan = st.session.rebase.view.as_mut().unwrap().plan.as_mut().unwrap();
    plan.upstream = glowfly_core::rebase::Upstream::Commit(head);
    plan.onto_label = "feature/long-running-integration-of-the-new-storage-backend".into();
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

/// Where the frame last painted `text`'s top, if it did.
fn painted_top(h: &Harness<'_, (AppState, Vec<Msg>)>, text: &str) -> Option<f32> {
    fn walk(s: &Shape, text: &str, out: &mut Option<f32>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, text, out)),
            Shape::Text(t) if t.galley.job.text == text => *out = Some(t.pos.y),
            _ => {}
        }
    }
    let mut out = None;
    h.output().shapes.iter().for_each(|c| walk(&c.shape, text, &mut out));
    out
}

/// Drags the first row two rows down and returns how far "Add app skeleton" (the second row,
/// which makes way upwards) has moved a frame after the drag reached its slot, and once settled.
fn moved_apart(reduce: bool, name: &str) -> (f32, f32) {
    use RebaseAction::*;
    let (mut st, dir) = with_panel(ThemeChoice::Dark, name, &[Pick, Pick, Pick, Pick, Pick, Pick]);
    st.settings.reduce_motion = reduce;
    // 20 ms frames, so a 120 ms move shows partway.
    let mut h = harness_stepped(st, true, 0.02);
    h.run();
    let at_rest = painted_top(&h, "Add app skeleton").expect("the row's summary");
    let first = h.get_by_label("Initial commit").bounding_box().unwrap();
    let pitch = h.get_by_label("Add app skeleton").bounding_box().unwrap().y0 - first.y0;
    let start = pos2(first.x0 as f32 + 16.0, ((first.y0 + first.y1) / 2.0) as f32);
    pointer(&mut h, start, Some(true));
    h.step();
    pointer(&mut h, start + egui::vec2(0.0, 12.0), None);
    h.step();
    pointer(&mut h, start + egui::vec2(0.0, 2.0 * pitch as f32 + 6.0), None);
    h.step();
    // egui starts an animation on the frame that asks for it: measure one frame later.
    h.step();
    let one_frame = at_rest - painted_top(&h, "Add app skeleton").unwrap();
    h.run();
    let settled = at_rest - painted_top(&h, "Add app skeleton").unwrap();
    drop(h);
    remove_fixture(&dir);
    (one_frame, settled)
}

#[test]
fn rows_move_apart_over_a_moment_or_at_once_with_reduce_motion() {
    let (frame, settled) = moved_apart(false, "rebase-shift-motion");
    assert!(settled > 30.0, "the row makes way by a row's pitch: {settled}");
    assert!(frame > 0.0 && frame < settled, "partway after one frame: {frame} of {settled}");
    let (frame, settled_still) = moved_apart(true, "rebase-shift-still");
    assert_eq!(frame, settled_still, "at once with Reduce motion");
    assert_eq!(settled_still, settled);
}
