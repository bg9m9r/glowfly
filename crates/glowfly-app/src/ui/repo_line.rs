//! The window's top line: a 2 px strip across the full width of the window's top edge in the
//! active tab's repository colour, with the drawn title bar and with the system frame, and
//! while Preferences is open. None while the active tab has no colour. It sits on a layer above
//! the panels and under menus and dialog backdrops.

use super::theme::color;
use crate::state::AppState;
use crate::state::repo_colours::active_colour;
use eframe::egui::{self, Id, LayerId, Order, Rect, vec2};

/// The line's height.
const LINE_H: f32 = 2.0;

/// Paints the line for the active tab, if it has a colour.
pub(super) fn paint(ctx: &egui::Context, st: &AppState) {
    let Some(c) = active_colour(st) else { return };
    let screen = ctx.screen_rect();
    let line = Rect::from_min_size(screen.min, vec2(screen.width(), LINE_H));
    ctx.layer_painter(LayerId::new(Order::Middle, Id::new("repo-line"))).rect_filled(line, 0.0, color::repo_hue(c));
}
