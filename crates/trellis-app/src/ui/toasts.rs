//! The toasts in the corner chosen in Preferences, anchored to the window's chrome: below the
//! last top panel or above the status bar (`ToastBounds`, measured by `frame_layout`), all one
//! width so a stack lines up on both edges.

use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use eframe::egui::{self, Align2, Rect, RichText, Stroke, Vec2, vec2};
use trellis_core::settings::Corner;

/// Where the window's chrome ends: the bottom of the last top panel and the top of the status
/// bar, in screen points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ToastBounds {
    pub top: f32,
    pub bottom: f32,
}

/// Every toast frame's outer width; its text wraps inside.
pub const TOAST_W: f32 = 360.0;
/// Between a toast and the chrome or window edge beside it.
pub const TOAST_MARGIN: f32 = 12.0;
/// Between two stacked toasts.
const TOAST_GAP: f32 = 6.0;
/// The frame's inner margin, left and right.
const PAD_X: i8 = 12;

/// The toast area's anchor in `corner`, `TOAST_MARGIN` inside the chrome `b` and the `screen`'s
/// edges.
pub(super) fn anchor(corner: Corner, b: ToastBounds, screen: Rect) -> (Align2, Vec2) {
    const M: f32 = TOAST_MARGIN;
    let top = (b.top - screen.top() + M).max(M);
    let bottom = -(screen.bottom() - b.bottom) - M;
    match corner {
        Corner::TopLeft => (Align2::LEFT_TOP, vec2(M, top)),
        Corner::TopRight => (Align2::RIGHT_TOP, vec2(-M, top)),
        Corner::BottomLeft => (Align2::LEFT_BOTTOM, vec2(M, bottom)),
        Corner::BottomRight => (Align2::RIGHT_BOTTOM, vec2(-M, bottom)),
    }
}

/// The toasts, newest last. The area takes clicks only while a toast has an action button, so
/// plain toasts let clicks through to what is under them.
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, bounds: ToastBounds, msgs: &mut Vec<Msg>) {
    st.toasts.retain(|t| t.at.elapsed() < t.lifetime());
    if st.toasts.is_empty() {
        return;
    }
    let (align, offset) = anchor(st.settings.notification_location, bounds, ctx.screen_rect());
    let interactable = st.toasts.iter().any(|t| t.action.is_some());
    egui::Area::new(egui::Id::new("toasts")).anchor(align, offset).interactable(interactable).show(ctx, |ui| {
        for t in &st.toasts {
            // Fade out over the last second.
            let fade = 1.0 - (t.at.elapsed().as_secs_f32() - (t.lifetime().as_secs_f32() - 1.0)).clamp(0.0, 1.0);
            let accent = if t.action.is_some() {
                color::blue()
            } else if t.ok {
                color::green()
            } else {
                color::red()
            };
            egui::Frame::new()
                .fill(color::toolbar().gamma_multiply(fade))
                .stroke(Stroke::new(1.0_f32, accent.gamma_multiply(fade)))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(PAD_X, 8))
                .show(ui, |ui| {
                    ui.set_width(TOAST_W - 2.0 * f32::from(PAD_X));
                    ui.add(egui::Label::new(RichText::new(&t.text).color(color::text().gamma_multiply(fade))).wrap());
                    if let Some(a) = &t.action {
                        ui.add_space(4.0);
                        ui.scope(|ui| {
                            ui.multiply_opacity(fade);
                            let button = egui::Button::new(&a.label).fill(color::input()).stroke(Stroke::new(1.0_f32, color::border()));
                            if ui.add(button).clicked() {
                                msgs.push(Msg::ToastAction(t.id));
                            }
                        });
                    }
                });
            ui.add_space(TOAST_GAP);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    #[test]
    fn anchor_sits_12px_inside_the_chrome() {
        let b = ToastBounds { top: 120.0, bottom: 794.0 };
        let screen = Rect::from_min_max(pos2(0.0, 0.0), pos2(1400.0, 820.0));
        let below = -(820.0 - 794.0) - 12.0;
        assert_eq!(anchor(Corner::TopLeft, b, screen), (Align2::LEFT_TOP, vec2(12.0, 132.0)));
        assert_eq!(anchor(Corner::TopRight, b, screen), (Align2::RIGHT_TOP, vec2(-12.0, 132.0)));
        assert_eq!(anchor(Corner::BottomLeft, b, screen), (Align2::LEFT_BOTTOM, vec2(12.0, below)));
        assert_eq!(anchor(Corner::BottomRight, b, screen), (Align2::RIGHT_BOTTOM, vec2(-12.0, below)));
        // No chrome above: still the margin below the window's top.
        let none = ToastBounds { top: -40.0, bottom: 820.0 };
        assert_eq!(anchor(Corner::TopLeft, none, screen).1, vec2(12.0, 12.0));
    }
}
