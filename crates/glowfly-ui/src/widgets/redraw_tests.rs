//! The shared animation pace: a frame after `REDRAW` while seen, none while clipped away, the
//! window minimized or its inner rect empty. Time is the harness's own clock.

use super::*;
use eframe::egui::{self, ViewportId, vec2};
use egui_kittest::Harness;

/// A harness whose one view calls `next_frame` on its rect (clipped to `clip` points wide when
/// given), or `wake` when `wake_only`.
fn harness(clip: Option<f32>, wake_only: bool) -> Harness<'static> {
    crate::snapshot_check::builder().with_size(vec2(120.0, 40.0)).with_step_dt(1.0 / 60.0).build_ui(move |ui| {
        if let Some(w) = clip {
            let r = ui.max_rect();
            ui.set_clip_rect(Rect::from_min_size(r.right_top() + vec2(10.0, 0.0), vec2(w, w)));
        }
        if wake_only {
            wake(ui.ctx());
        } else {
            next_frame(ui, ui.max_rect());
        }
    })
}

fn delay(h: &Harness) -> Duration {
    h.output().viewport_output[&ViewportId::ROOT].repaint_delay
}

fn wants_frame(h: &Harness) -> bool {
    delay(h) < Duration::from_secs(1)
}

fn set_viewport(h: &mut Harness, f: impl FnOnce(&mut egui::ViewportInfo)) {
    f(h.input_mut().viewports.entry(ViewportId::ROOT).or_default());
}

#[test]
fn a_visible_thing_asks_for_a_frame_after_the_pace() {
    let mut h = harness(None, false);
    h.run();
    let d = delay(&h);
    assert!(d > Duration::ZERO && d <= REDRAW, "{d:?}");
}

#[test]
fn a_clipped_away_thing_asks_for_nothing() {
    let mut h = harness(Some(0.0), false);
    h.run();
    assert!(!wants_frame(&h), "{:?}", delay(&h));
}

#[test]
fn a_minimized_window_asks_for_nothing() {
    let mut h = harness(None, false);
    h.run();
    set_viewport(&mut h, |v| v.minimized = Some(true));
    h.step();
    assert!(!wants_frame(&h), "{:?}", delay(&h));
    let mut h = harness(None, true);
    h.run();
    assert!(wants_frame(&h), "wake asks while shown");
    set_viewport(&mut h, |v| v.minimized = Some(true));
    h.step();
    assert!(!wants_frame(&h), "wake asks for nothing minimized: {:?}", delay(&h));
}

#[test]
fn an_empty_inner_rect_counts_as_hidden() {
    let mut h = harness(None, false);
    h.run();
    set_viewport(&mut h, |v| v.inner_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(0.0, 300.0))));
    h.step();
    assert!(!wants_frame(&h), "{:?}", delay(&h));
}
