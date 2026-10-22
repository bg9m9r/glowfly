//! The toasts in the corner chosen in Preferences, anchored to the window's chrome: below the
//! last top panel or above the status bar (`ToastBounds`, measured by `frame_layout`), all one
//! width so a stack lines up on both edges. Each floats on the overlay with `LG` corners and the
//! Float shadow, outlined in its outcome's tone, with a small x top right that closes it. The
//! pointer over a toast holds it (`NotifyMsg::HoldToast`): it neither fades nor expires.

use super::theme::color;
use super::theme::surfaces::Tone;
use super::theme::tokens::{self, Elevation, radius};
use super::widgets::{self, buttons};
use crate::msg::{Msg, NotifyMsg};
use crate::state::AppState;
use eframe::egui::{self, Align, Align2, Layout, Rect, RichText, Stroke, Vec2, vec2};
use glowfly_core::settings::Corner;

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
/// What the x takes beside the text: its 20 px box and a 4 px gap.
const CLOSE_SLOT: f32 = 24.0;

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

/// The toasts, newest last. Every toast has its x, so the area always takes clicks. A failure's
/// fix and its Details sit side by side under the text, the fix first. They fade and expire by
/// the frame's time stamp; without one (snapshot tests) they stay at full strength.
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, bounds: ToastBounds, msgs: &mut Vec<Msg>) {
    let now = st.view.frame_at;
    if let Some(now) = now {
        st.expire_toasts(now);
    }
    if st.toasts.is_empty() {
        return;
    }
    let (align, offset) = anchor(st.settings.notification_location, bounds, ctx.screen_rect());
    let p = color::current();
    let (lang, held) = (st.settings.lang(), st.held_toast());
    let mut over = None;
    egui::Area::new(egui::Id::new("toasts")).anchor(align, offset).interactable(true).show(ctx, |ui| {
        for t in &st.toasts {
            let fade = now.map_or(1.0, |n| t.opacity(n));
            // A failure stays red with a button too (Details); other toasts with one are blue.
            let tone = if !t.ok {
                Tone::Danger
            } else if t.action.is_some() {
                Tone::Info
            } else {
                Tone::Success
            };
            let mut shadow = tokens::shadow(Elevation::Float, &p);
            shadow.color = shadow.color.gamma_multiply(fade);
            let frame = egui::Frame::new()
                .fill(p.overlay().gamma_multiply(fade))
                .stroke(Stroke::new(1.0_f32, p.tone(tone).gamma_multiply(fade)))
                .corner_radius(radius::LG)
                .shadow(shadow)
                .inner_margin(egui::Margin::symmetric(PAD_X, 8))
                .show(ui, |ui| {
                    ui.set_width(TOAST_W - 2.0 * f32::from(PAD_X));
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = CLOSE_SLOT - 20.0;
                        ui.vertical(|ui| {
                            ui.set_width(TOAST_W - 2.0 * f32::from(PAD_X) - CLOSE_SLOT);
                            ui.add(egui::Label::new(RichText::new(&t.text).color(color::text().gamma_multiply(fade))).wrap());
                        });
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            ui.scope(|ui| {
                                ui.multiply_opacity(fade);
                                if widgets::remove_icon(ui, lang.tr("common.dismiss.button")).clicked() {
                                    msgs.push(Msg::Notify(NotifyMsg::CloseToast(t.id)));
                                }
                            });
                        });
                    });
                    if t.action.is_some() || t.details.is_some() {
                        ui.add_space(4.0);
                        ui.scope(|ui| {
                            ui.multiply_opacity(fade);
                            ui.horizontal(|ui| {
                                let buttons = [(&t.action, Msg::ToastAction(t.id)), (&t.details, Msg::ToastDetails(t.id))];
                                for (a, msg) in buttons {
                                    let Some(a) = a else { continue };
                                    if ui.add(buttons::secondary(&a.label).small()).clicked() {
                                        msgs.push(msg);
                                    }
                                }
                            });
                        });
                    }
                });
            if frame.response.contains_pointer() {
                over = Some(t.id);
            }
            ui.add_space(TOAST_GAP);
        }
    });
    if over != held {
        msgs.push(Msg::Notify(NotifyMsg::HoldToast(over)));
    }
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
