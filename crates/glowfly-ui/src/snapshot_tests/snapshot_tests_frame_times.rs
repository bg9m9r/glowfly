//! The frame-time overlay (`frame_overlay.rs`): drawn in the bottom-left corner when
//! Preferences > General > Diagnostics turns it on, and not at all by default.

use super::show;
use crate::state::AppState;
use crate::state::frame_times::FrameSummary;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::settings;
use crate::ui::theme::{self, color};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

const SHOWN: &str = "Frame 3.2 ms · average 2.1 ms · slowest 12.0 ms";

/// A small window drawing only the overlay for `st`, in the dark Garden palette like the other
/// view snapshots.
fn harness(on: bool) -> Harness<'static, AppState> {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.settings.show_frame_times = on;
    st.view.frame_times = FrameSummary { last_ms: 3.2, avg_ms: 2.1, max_ms: 12.0 };
    let p = theme::GARDEN_DARK;
    crate::ui::snapshot_check::builder().with_size(egui::vec2(440.0, 90.0)).build_state(
        move |ctx, st: &mut AppState| {
            color::set_palette(&p);
            theme::apply_style(ctx, &p);
            ctx.set_theme(theme::egui_theme(&p));
            egui::CentralPanel::default().show(ctx, |_| {});
            show(ctx, st);
        },
        st,
    )
}

#[test]
fn frame_times_overlay() {
    let mut h = harness(true);
    h.run();
    assert!(h.query_by_label(SHOWN).is_some());
    h.check_snapshot("frame_times_overlay");
}

#[test]
fn frame_times_are_hidden_by_default() {
    let mut h = harness(false);
    h.run();
    assert!(h.query_by_label_contains("slowest").is_none());
}
