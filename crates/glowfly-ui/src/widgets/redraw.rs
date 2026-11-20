//! The one pace for animations that run while work does (the dots, the firefly loader, spinners,
//! a fading toast): about 30 frames a second, and none while unseen.

use eframe::egui::{Context, Rect, Ui};
use std::time::Duration;

/// How often something moving asks for its next frame (about 30 a second). egui keeps the
/// earliest request of a frame, so several moving things still give one frame per `REDRAW`.
pub const REDRAW: Duration = Duration::from_millis(33);

/// Asks for a frame `after` from now. egui takes its predicted frame time (`predicted_dt`, 1/60 s
/// unless a backend sets it) off every delay so a frame doesn't overshoot; that would halve the
/// 33 ms pace on a fast display, so it is added back here.
pub fn after(ctx: &Context, after: Duration) {
    let predicted = Duration::try_from_secs_f32(ctx.input(|i| i.predicted_dt)).unwrap_or_default();
    ctx.request_repaint_after(after + predicted);
}

/// Asks for the next paced frame: `after` with `REDRAW`.
pub fn pace(ctx: &Context) {
    after(ctx, REDRAW);
}

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
        pace(ui.ctx());
    }
    seen
}

/// Asks for a frame after `REDRAW` unless the window is hidden, for what has no one rect to
/// check (the tab strip's running dots).
pub fn wake(ctx: &Context) {
    if !window_hidden(ctx) {
        pace(ctx);
    }
}

#[cfg(test)]
#[path = "redraw_tests.rs"]
mod tests;
