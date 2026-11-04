//! The popover's placement and layout: unjustified contents at the set width (issue #523's
//! letter-spaced text), opened below its anchor and kept `EDGE` inside the window.

use super::*;
use eframe::egui::{Sense, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const SCREEN: egui::Vec2 = vec2(600.0, 400.0);
const WIDTH: f32 = 420.0;
const LONG: &str = "Conflicts with origin/release-please--branches--main--components--glowfly";

/// What the popover's contents saw: whether their layout justifies, and the popover's rect.
#[derive(Default)]
struct Seen {
    justified: Option<bool>,
    rect: Option<Rect>,
}

/// A harness with a 30 px anchor at `anchor_x` whose popover is open, holding a long title.
fn harness(anchor_x: f32) -> Harness<'static, Seen> {
    let mut h = crate::snapshot_check::builder().with_size(SCREEN).build_ui_state(
        move |ui, seen: &mut Seen| {
            let id = Id::new("test-popover");
            let anchor = ui.allocate_rect(Rect::from_min_size(pos2(anchor_x, 10.0), vec2(30.0, 24.0)), Sense::click());
            ui.memory_mut(|m| m.open_popup(id));
            below(ui, id, &anchor, WIDTH, PopupCloseBehavior::IgnoreClicks, |ui| {
                seen.justified = Some(ui.layout().horizontal_justify());
                ui.label(LONG);
                seen.rect = Some(ui.max_rect());
            });
        },
        Seen::default(),
    );
    h.run();
    h
}

/// The frame's outer rect from the contents' rect.
fn outer(r: Rect) -> Rect {
    r.expand(PAD as f32 + 1.0)
}

#[test]
fn contents_are_not_justified() {
    // A justified layout spreads a wrapped row's glyphs across the width.
    let h = harness(500.0);
    assert_eq!(h.state().justified, Some(false));
    let r = outer(h.state().rect.expect("drawn"));
    assert!((r.width() - WIDTH).abs() < 0.5, "the set width, not the anchor's or the text's: {}", r.width());
    assert!((r.right() - 530.0).abs() < 0.5, "right-aligned with its anchor: {r:?}");
    assert!((r.top() - (34.0 + GAP)).abs() < 0.5, "{GAP} px below its anchor: {r:?}");
}

#[test]
fn near_the_window_s_edges_it_stays_inside() {
    let r = outer(harness(SCREEN.x - 30.0).state().rect.expect("drawn"));
    assert!((r.right() - (SCREEN.x - EDGE)).abs() < 0.5, "{EDGE} px inside the right edge: {r:?}");
    let r = outer(harness(10.0).state().rect.expect("drawn"));
    assert!((r.left() - EDGE).abs() < 0.5, "{EDGE} px inside the left edge: {r:?}");
}

#[test]
fn a_narrow_window_narrows_it() {
    let screen = Rect::from_min_max(pos2(EDGE, EDGE), pos2(300.0 - EDGE, 800.0 - EDGE));
    let anchor = Rect::from_min_size(pos2(250.0, 20.0), vec2(30.0, 24.0));
    assert_eq!(place(anchor, screen.width(), screen).x, EDGE);
}

/// A harness with an anchor, a button with a tooltip beside it in the same layer (a toolbar's),
/// and the anchor's popover open when `open`; the pointer rests on the button.
fn tooltip_shows(open: bool) -> bool {
    let mut h = crate::snapshot_check::builder().with_size(SCREEN).build_ui(move |ui| {
        let id = Id::new("test-popover");
        let anchor = ui.allocate_rect(Rect::from_min_size(pos2(500.0, 10.0), vec2(30.0, 24.0)), Sense::click());
        let _ = ui.put(Rect::from_min_size(pos2(20.0, 10.0), vec2(80.0, 24.0)), egui::Button::new("Other")).on_hover_text("Other's tip");
        if open {
            ui.memory_mut(|m| m.open_popup(id));
        }
        below(ui, id, &anchor, WIDTH, PopupCloseBehavior::IgnoreClicks, |ui| {
            ui.label(LONG);
        });
    });
    h.run();
    h.get_by_label("Other").hover();
    for _ in 0..4 {
        h.run();
    }
    h.query_by_label("Other's tip").is_some()
}

#[test]
fn an_open_popover_hides_its_layer_s_tooltips() {
    assert!(tooltip_shows(false), "the tooltip shows while the popover is closed");
    assert!(!tooltip_shows(true), "and not over the open popover");
}
