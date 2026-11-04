//! The firefly loader (design pass D20): three small dots in `lantern` that light up one after
//! another, for a pane or a row that is loading. The words it replaces stay as its AccessKit
//! label. Under reduce motion the dots stand still at 60%. While it moves it asks for the next
//! frame only when the fade next changes, and not at all while hidden (scrolled out of view,
//! clipped away or the window minimized), so an idle window stops repainting.

use super::super::theme::color::{self, Palette};
use super::super::theme::contrast;
use super::super::theme::tokens::motion;
use eframe::egui::{Painter, Pos2, Rect, Response, Sense, Ui, Vec2, WidgetInfo, WidgetType, emath::GuiRounding, pos2, vec2};
use std::time::Duration;

/// A dot's diameter and the distance between dot centres.
const DOT: f32 = 4.0;
const PITCH: f32 = 8.0;
/// One pass along the three dots, in seconds.
const PERIOD: f64 = 1.2;
/// The faintest a dot gets between its turns.
const LOW: f32 = 0.25;
/// The strength of every dot held still (reduce motion), where it keeps 3:1 on the surfaces a
/// loader sits on; full `lantern` where it wouldn't (the light themes).
const STILL: f32 = 0.6;
/// The contrast a still dot keeps on bg, panel and raised.
const STILL_FLOOR: f64 = 3.0;
/// The fade is redrawn this often while it moves (egui wakes for it; nothing repaints between).
const REDRAW: Duration = Duration::from_millis(33);
/// Where in its cycle a dot has risen, starts to fall and has fallen: it holds full for a third
/// of the cycle, so one of the three is always at full `lantern`.
const RISEN: f64 = 0.25;
const FALLS: f64 = 7.0 / 12.0;
const FALLEN: f64 = 5.0 / 6.0;

/// The loader's width and height.
pub const SIZE: Vec2 = vec2(2.0 * PITCH + DOT, DOT);

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Dot `i`'s strength at `time` (the context's clock, in seconds): each dot rises, holds, falls
/// and rests, a third of the cycle after the one before.
fn strength(time: f64, i: usize) -> f32 {
    let phase = (time / PERIOD - i as f64 / 3.0).rem_euclid(1.0);
    let lit = if phase < RISEN {
        smoothstep(phase / RISEN)
    } else if phase < FALLS {
        1.0
    } else {
        1.0 - smoothstep((phase - FALLS) / (FALLEN - FALLS))
    };
    LOW + (1.0 - LOW) * lit as f32
}

/// How strong the still dots are in palette `p`: `STILL` where `lantern` at that strength keeps
/// `STILL_FLOOR` on bg, panel and raised, else full.
fn still_strength(p: &Palette) -> f32 {
    let keeps = [p.bg, p.panel, p.raised].iter().all(|&s| contrast::ratio(contrast::mix(s, p.lantern, STILL), s) >= STILL_FLOOR);
    if keeps { STILL } else { 1.0 }
}

/// The three dots' strengths at `time`; None holds them still at `still`.
fn alphas(time: Option<f64>, still: f32) -> [f32; 3] {
    std::array::from_fn(|i| time.map_or(still, |t| strength(t, i)))
}

/// Paints the three dots centred on `c` in palette `pal`'s `lantern`; `time` moves the fade,
/// None holds them still.
pub fn paint(p: &Painter, c: Pos2, pal: &Palette, time: Option<f64>) {
    // On whole pixels: the dots are an even number of pixels across, so each then fills whole
    // pixels with the same soft edge on every side.
    let c = c.round_to_pixels(p.pixels_per_point());
    let col = pal.lantern;
    for (i, alpha) in alphas(time, still_strength(pal)).into_iter().enumerate() {
        p.circle_filled(pos2(c.x + (i as f32 - 1.0) * PITCH, c.y), DOT / 2.0, col.gamma_multiply(alpha));
    }
}

/// Whether a loader at `rect` would be seen: inside `ui`'s clip and the window not minimized.
fn visible(ui: &Ui, rect: Rect) -> bool {
    ui.is_rect_visible(rect) && !ui.input(|i| i.viewport().minimized.unwrap_or(false))
}

/// Paints the loader centred on `c` with `ui`'s painter, still under reduce motion
/// (`motion::reduced`). While it moves and can be seen, asks for a frame after `REDRAW`.
pub fn paint_at(ui: &Ui, c: Pos2) {
    let rect = Rect::from_center_size(c, SIZE);
    let moving = !motion::reduced(ui.ctx()) && visible(ui, rect);
    paint(ui.painter(), c, &color::current(), moving.then(|| ui.input(|i| i.time)));
    if moving {
        ui.ctx().request_repaint_after(REDRAW);
    }
}

/// Lays out the loader in `ui` and paints it, read out as `label` (the words it stands for).
pub fn show(ui: &mut Ui, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(SIZE, Sense::hover());
    paint_at(ui, rect.center());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, label));
    resp
}

#[cfg(test)]
#[path = "fireflies_tests.rs"]
mod tests;
