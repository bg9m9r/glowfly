//! The capped spinner: it draws egui's arc at the same moment, and asks for frames only at the
//! shared pace while it can be seen.

use super::*;
use eframe::egui::{self, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::time::Duration;

fn harness(clip: Option<f32>) -> Harness<'static> {
    crate::snapshot_check::builder().with_size(vec2(120.0, 40.0)).with_step_dt(1.0 / 60.0).build_ui(move |ui| {
        if let Some(w) = clip {
            let r = ui.max_rect();
            ui.set_clip_rect(Rect::from_min_size(r.right_top() + vec2(10.0, 0.0), vec2(w, w)));
        }
        ui.add(Spinner::new().size(14.0));
    })
}

fn delay(h: &Harness) -> Duration {
    h.output().viewport_output[&ViewportId::ROOT].repaint_delay
}

#[test]
fn it_draws_egui_s_arc_at_the_same_moment() {
    let c = Color32::from_rgb(200, 100, 50);
    let mut ours = crate::snapshot_check::builder().with_size(vec2(120.0, 40.0)).with_step_dt(1.0 / 60.0).build_ui(move |ui| {
        ui.add(Spinner::new().size(14.0).color(c));
    });
    let mut theirs = crate::snapshot_check::builder().with_size(vec2(120.0, 40.0)).with_step_dt(1.0 / 60.0).build_ui(move |ui| {
        ui.add(egui::Spinner::new().size(14.0).color(c));
    });
    // The harnesses settle in different numbers of frames, so set the clock itself.
    for h in [&mut ours, &mut theirs] {
        h.input_mut().time = Some(1.7);
        h.step();
    }
    assert_eq!(format!("{:?}", ours.output().shapes), format!("{:?}", theirs.output().shapes));
}

#[test]
fn it_asks_for_a_frame_after_the_redraw_pace() {
    let mut h = harness(None);
    // egui's own spinner would repaint at once every frame and exceed the step limit.
    h.run();
    let d = delay(&h);
    assert!(d > Duration::ZERO && d <= redraw::REDRAW, "{d:?}");
    assert!(h.query_by_role(egui::accesskit::Role::ProgressIndicator).is_some(), "it stays a progress indicator");
}

#[test]
fn clipped_away_it_asks_for_nothing() {
    let mut h = harness(Some(0.0));
    h.run();
    assert!(delay(&h) >= Duration::from_secs(1), "{:?}", delay(&h));
}

#[test]
fn in_a_minimized_window_it_asks_for_nothing() {
    let mut h = harness(None);
    h.run();
    h.input_mut().viewports.entry(ViewportId::ROOT).or_default().minimized = Some(true);
    h.step();
    assert!(delay(&h) >= Duration::from_secs(1), "{:?}", delay(&h));
}
