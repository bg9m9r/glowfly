//! The frame-time overlay (Preferences > General > Diagnostics > Frame times): the last,
//! average and slowest of the recent frames in the window's bottom-left corner, from the
//! times the shell keeps (`state/frame_times.rs`), drawn by the shell (`app_frame.rs`) over
//! everything else. Off by default; it doesn't take clicks.

use crate::state::AppState;
use crate::ui::theme::color;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align2, FontId, RichText, Stroke, vec2};

pub(super) fn show(ctx: &egui::Context, st: &AppState) {
    if !st.settings.show_frame_times {
        return;
    }
    let f = st.view.frame_times;
    let one = |ms: f32| format!("{ms:.1}");
    let text =
        st.settings.lang().trf("frame_times.overlay", &[("last", &one(f.last_ms)), ("avg", &one(f.avg_ms)), ("max", &one(f.max_ms))]);
    let p = color::current();
    egui::Area::new(egui::Id::new("frame-times"))
        .anchor(Align2::LEFT_BOTTOM, vec2(12.0, -40.0))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.overlay())
                .stroke(Stroke::new(1.0_f32, p.overlay_edge()))
                .corner_radius(radius::MD)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(text).font(FontId::monospace(type_scale::SMALL)).color(p.weak)).extend());
                });
        });
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_frame_times.rs"]
mod snapshot_tests;
