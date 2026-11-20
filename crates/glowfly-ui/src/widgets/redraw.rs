//! The one pace for animations that run while work does (the dots, the firefly loader, spinners,
//! a fading toast): about 30 frames a second, and none while unseen.

use eframe::egui::{Context, Rect, Ui};
use std::time::Duration;

/// How often something moving asks for its next frame (about 30 a second). egui keeps the
/// earliest request of a frame, so several moving things still give one frame per `REDRAW`.
pub const REDRAW: Duration = Duration::from_millis(33);

/// Whether the window can't be seen: minimized, or with nothing inside (egui's `ViewportInfo`
/// has no occluded flag).
pub fn window_hidden(ctx: &Context) -> bool {
    ctx.input(|i| {
        let v = i.viewport();
        v.minimized == Some(true) || v.inner_rect.is_some_and(|r| r.width() <= 0.0 || r.height() <= 0.0)
    })
}

/// Whether something painted at `rect` would be seen: inside `ui`'s clip and the window shown.
pub fn seen(ui: &Ui, rect: Rect) -> bool {
    ui.is_rect_visible(rect) && !window_hidden(ui.ctx())
}

/// For something moving at `rect`: when it can be seen, asks for a frame after `REDRAW` and says
/// so.
pub fn next_frame(ui: &Ui, rect: Rect) -> bool {
    let seen = seen(ui, rect);
    if seen {
        ui.ctx().request_repaint_after(REDRAW);
    }
    seen
}

/// Asks for a frame after `REDRAW` unless the window is hidden, for what has no one rect to
/// check (the tab strip's running dots).
pub fn wake(ctx: &Context) {
    if !window_hidden(ctx) {
        ctx.request_repaint_after(REDRAW);
    }
}

#[cfg(test)]
#[path = "redraw_tests.rs"]
mod tests;
