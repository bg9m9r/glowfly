//! Three dots that say work is running (design pass D15): the status bar's job and a tab's state
//! slot. They pulse one after another, or stand still at full strength under reduce motion.

use eframe::egui::{Color32, Painter, Pos2, Ui, Vec2, pos2, vec2};

/// A dot's diameter and the distance between dot centres.
const DOT: f32 = 2.5;
const STEP: f32 = 4.0;
/// How long one pulse takes to pass along the three dots, in seconds.
const PERIOD: f64 = 1.2;
/// The faintest a dot gets mid-pulse.
const LOW: f32 = 0.35;

/// The dots' width and height.
pub const SIZE: Vec2 = vec2(2.0 * STEP + DOT, DOT);

/// Paints the dots centred on `c` in `col`; `time` (the context's, in seconds) moves the pulse,
/// None holds them still.
pub fn paint(p: &Painter, c: Pos2, col: Color32, time: Option<f64>) {
    for i in 0..3 {
        let alpha = time.map_or(1.0, |t| strength(t, i));
        p.circle_filled(pos2(c.x + (i as f32 - 1.0) * STEP, c.y), DOT / 2.0, col.gamma_multiply(alpha));
    }
}

/// Dot `i`'s strength at `time`: a smooth rise and fall, each dot a third of a period later.
fn strength(time: f64, i: usize) -> f32 {
    let phase = (time / PERIOD - i as f64 / 3.0).rem_euclid(1.0);
    let wave = 0.5 - 0.5 * (phase * std::f64::consts::TAU).cos();
    LOW + (1.0 - LOW) * wave as f32
}

/// Lays out the dots in `ui` (with `pad` either side) and paints them, asking for the next frame
/// while they move.
pub fn show(ui: &mut Ui, col: Color32, still: bool, pad: f32) {
    let (rect, _) = ui.allocate_exact_size(SIZE + vec2(2.0 * pad, 0.0), eframe::egui::Sense::hover());
    paint_at(ui, rect.center(), col, still);
}

/// Paints the dots centred on `c` with `ui`'s painter, asking for the next frame while they move.
pub fn paint_at(ui: &Ui, c: Pos2, col: Color32, still: bool) {
    let time = (!still).then(|| ui.input(|i| i.time));
    paint(ui.painter(), c, col, time);
    if !still {
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pulse_passes_along_the_dots() {
        assert!((strength(0.0, 0) - LOW).abs() < 1e-6, "the first dot starts at its faintest");
        assert!((strength(PERIOD / 2.0, 0) - 1.0).abs() < 1e-6, "and is full half a period on");
        assert!((strength(PERIOD / 3.0, 1) - LOW).abs() < 1e-6, "the second a third of a period later");
        for t in 0..50 {
            for i in 0..3 {
                let s = strength(f64::from(t) * 0.05, i);
                assert!((LOW..=1.0).contains(&s), "{s}");
            }
        }
    }
}
