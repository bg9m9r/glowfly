//! The build-status marker on pull request rows. Each state has its own shape as well as its own
//! colour, so it reads without colour vision; all painted, since the bundled fonts lack the glyphs.

use super::theme::color;
use eframe::egui::{Painter, Pos2, Stroke, vec2};
use trellis_core::forge::CheckState;

/// Width a PR row keeps free for the marker.
pub const MARKER_SLOT: f32 = 14.0;

/// Paints the check-state marker centred on `c`: a ticked disc (passed), a crossed disc
/// (failed) or a ring with a dot (pending).
pub fn paint_marker(p: &Painter, c: Pos2, state: CheckState) {
    let mark = Stroke::new(1.4_f32, color::panel());
    match state {
        CheckState::Passed => {
            p.circle_filled(c, 5.0, color::green());
            p.line(vec![c + vec2(-2.4, 0.2), c + vec2(-0.6, 2.0), c + vec2(2.6, -1.8)], mark);
        }
        CheckState::Failed => {
            p.circle_filled(c, 5.0, color::red());
            let h = 1.9;
            p.line_segment([c + vec2(-h, -h), c + vec2(h, h)], mark);
            p.line_segment([c + vec2(-h, h), c + vec2(h, -h)], mark);
        }
        CheckState::Pending => {
            p.circle_stroke(c, 4.2, Stroke::new(1.6_f32, color::amber()));
            p.circle_filled(c, 1.4, color::amber());
        }
    }
}
