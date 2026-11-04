//! The firefly loader: its fade passes along the dots, holds still under reduce motion, and asks
//! for frames only while it moves where it can be seen. Time is the harness's own clock, which
//! moves a fixed step per frame, so nothing here waits on the wall clock.

use super::*;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[test]
fn one_dot_is_always_at_full_strength() {
    for t in 0..240 {
        let t = f64::from(t) * 0.01;
        let top = (0..3).map(|i| strength(t, i)).fold(0.0_f32, f32::max);
        assert!((top - 1.0).abs() < 1e-6, "at {t}: {top}");
        for i in 0..3 {
            assert!((LOW..=1.0).contains(&strength(t, i)));
        }
    }
}

#[test]
fn the_fade_repeats_every_period_and_passes_along_the_dots() {
    for i in 0..3 {
        assert!((strength(0.37, i) - strength(0.37 + PERIOD, i)).abs() < 1e-5, "it repeats every period");
    }
    // Each dot reaches full a third of a period after the one before.
    let first_full = |i: usize| (0..1200).map(|ms| f64::from(ms) / 1000.0).find(|&t| (strength(t, i) - 1.0).abs() < 1e-6).unwrap();
    assert!(first_full(1) > first_full(0), "dot 1 peaks after dot 0");
    assert!((strength(0.3, 0) - 1.0).abs() < 1e-6 && strength(0.3, 1) < 1.0, "at 0.3 s the first is lit, the second rising");
}

/// A harness drawing one loader (`still` for reduce motion) inside a clip of `clip` points
/// (None: unclipped), read out as "Loading the blame…". Its clock moves a 60 Hz frame per step
/// (egui takes the frame time off a delayed repaint).
fn harness(still: bool, clip: Option<f32>) -> Harness<'static> {
    crate::ui::snapshot_check::builder().with_size(egui::vec2(120.0, 40.0)).with_step_dt(1.0 / 60.0).build_ui(move |ui| {
        // What `theme::a11y::configure` does under reduce motion.
        ui.ctx().all_styles_mut(|s| s.animation_time = motion::animation_time(still));
        if let Some(w) = clip {
            let r = ui.max_rect();
            ui.set_clip_rect(Rect::from_min_size(r.right_top() + vec2(10.0, 0.0), vec2(w, w)));
        }
        show(ui, "Loading the blame…");
    })
}

/// How long after the last frame it asked for the next.
fn delay(h: &Harness) -> Duration {
    h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay
}

/// Whether the last frame asked for another soon (egui's "no repaint" is a delay of years).
fn wants_frame(h: &Harness) -> bool {
    delay(h) < Duration::from_secs(1)
}

#[test]
fn moving_it_asks_for_a_frame_at_the_next_step_not_at_once() {
    let mut h = harness(false, None);
    // `run` stops at the first frame that asks for no immediate repaint: a loader asking for
    // one every frame would exceed the step limit instead.
    h.run();
    let delay = delay(&h);
    assert!(delay > Duration::ZERO && delay <= STEP, "{delay:?}");
    assert!(h.query_by_label("Loading the blame…").is_some(), "the words stay as its label");
}

#[test]
fn under_reduce_motion_it_stands_still_and_asks_for_nothing() {
    let mut h = harness(true, None);
    h.run();
    assert!(!wants_frame(&h), "{:?}", delay(&h));
}

#[test]
fn hidden_it_asks_for_nothing() {
    let mut h = harness(false, Some(0.0));
    h.run();
    assert!(!wants_frame(&h), "clipped away: {:?}", delay(&h));
}

#[test]
fn in_a_minimized_window_it_asks_for_nothing() {
    let mut h = harness(false, None);
    h.run();
    h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().minimized = Some(true);
    h.step();
    assert!(!wants_frame(&h), "{:?}", delay(&h));
}

#[test]
fn still_dots_are_all_at_sixty_percent() {
    assert_eq!(alphas(None), [STILL; 3]);
    assert_eq!(STILL, 0.6);
    let moving = alphas(Some(PERIOD / 3.0));
    assert!(moving[0] > moving[1] && moving[0] > moving[2], "{moving:?}");
}
