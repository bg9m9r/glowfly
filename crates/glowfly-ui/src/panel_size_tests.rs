use super::*;
use crate::state::panel_layout::DETAILS;
use glowfly_core::settings::Settings;
use std::time::Instant;

fn input(w: f32, h: f32) -> egui::RawInput {
    egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h))), ..Default::default() }
}

/// One frame of a right-hand drawer with short content (nothing in it spans the width), the
/// way `details::show` draws it; returns the drawn width.
fn frame(ctx: &egui::Context, st: &mut AppState, w: f32) -> f32 {
    let mut width = 0.0;
    let mut msgs = Vec::new();
    let _ = ctx.run(input(w, 800.0), |ctx| {
        let (_, fit) = side_fits(ctx, st);
        let fit = fit.unwrap();
        let id = Id::new(DETAILS_ID);
        pin(ctx, id, fit.size, false);
        let resp = side(egui::SidePanel::right(id), fit).show(ctx, |ui| {
            fill(ui);
            ui.label("short")
        });
        track(ctx, st, SizedPanel::Details, id, resp.response.rect.width(), &mut msgs);
        width = resp.response.rect.width();
    });
    width
}

fn state() -> AppState {
    AppState::new(Settings { show_left_panel: false, ..Settings::default() }, Vec::new(), Instant::now())
}

#[test]
fn a_tiny_size_left_in_egui_memory_is_repaired() {
    let ctx = egui::Context::default();
    let mut st = state();
    let id = Id::new(DETAILS_ID);
    ctx.data_mut(|d| d.insert_persisted(id, PanelState { rect: Rect::from_min_size(Pos2::ZERO, egui::vec2(120.0, 800.0)) }));
    assert_eq!(frame(&ctx, &mut st, 2000.0), DETAILS.default_for(2000.0));
}

#[test]
fn short_content_does_not_shrink_the_drawer_over_frames() {
    let ctx = egui::Context::default();
    let mut st = state();
    for _ in 0..4 {
        assert_eq!(frame(&ctx, &mut st, 1600.0), 400.0);
    }
}

#[test]
fn a_saved_too_small_width_is_drawn_at_the_minimum() {
    let ctx = egui::Context::default();
    let mut st = state();
    st.settings.panel_sizes.details = Some(120.0);
    assert_eq!(frame(&ctx, &mut st, 2000.0), DETAILS.min);
}

#[test]
fn the_drawer_comes_back_after_the_window_shrinks_and_grows() {
    let ctx = egui::Context::default();
    let mut st = state();
    st.settings.panel_sizes.details = Some(600.0);
    assert_eq!(frame(&ctx, &mut st, 2000.0), 600.0);
    let small = frame(&ctx, &mut st, 700.0);
    assert!(small <= 700.0 - MAIN_MIN_WIDTH, "the main view keeps its room: {small}");
    assert_eq!(frame(&ctx, &mut st, 2000.0), 600.0);
    assert_eq!(st.settings.panel_sizes.details, Some(600.0), "the saved width is never overwritten by a resize");
}

#[test]
fn side_fits_share_the_width_between_the_shown_drawers() {
    let ctx = egui::Context::default();
    let mut st = state();
    st.settings.show_left_panel = true;
    let _ = ctx.run(input(1400.0, 800.0), |ctx| {
        let (l, d) = side_fits(ctx, &st);
        assert_eq!((l.map(|f| f.size), d.map(|f| f.size)), (Some(260.0), Some(400.0)));
    });
    st.settings.show_details_panel = false;
    let _ = ctx.run(input(1400.0, 800.0), |ctx| {
        let (l, d) = side_fits(ctx, &st);
        assert_eq!((l.map(|f| f.size), d), (Some(260.0), None));
    });
}
