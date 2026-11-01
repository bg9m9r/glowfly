//! A segment bar: several targets laid out flush as one shape, each with its own response,
//! state and focus. A framed bar (the toolbar's Repository › Branch pickers) paints its fill
//! and 1 px border, full-height 1 px dividers between its segments, and each segment's state
//! over the segment's whole shape, its corners the bar's inner ones on the outer ends and square
//! where segments meet; a framed bar of one segment is a single framed control, whose border
//! goes `weak` while it is lit. A quiet bar (a toolbar split button) has no frame: its
//! divider always shows, and only the hovered segment lights. A disabled segment never lights.
//! Every state is painted by `button_look::paint_target`.
//!
//! The bar is laid out first (`segment_bar`), so the caller can open a menu from a segment's
//! response and learn which segment is latched, then painted (`Bar::paint`) before the caller
//! draws each segment's content over it.

use super::button_look::{Press, Target, paint_target};
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens::{BORDER_W, DIVIDER_INSET, radius};
use eframe::egui::{Color32, CornerRadius, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

/// How a bar is drawn.
#[derive(Clone, Copy, Debug)]
pub enum BarStyle {
    /// A container with `fill` and a 1 px border, with full-height dividers.
    Framed { fill: Color32 },
    /// No container; the dividers are `divider` px tall, centred.
    Quiet { divider: f32 },
}

/// One segment: its width and whether it can be clicked. A disabled segment still senses hover
/// and focus, so it can explain itself.
#[derive(Clone, Copy, Debug)]
pub struct Seg {
    pub w: f32,
    pub enabled: bool,
}

/// A laid-out bar: one response per segment, left to right.
pub struct Bar {
    pub responses: Vec<Response>,
    rect: Rect,
    outer: u8,
    style: BarStyle,
    size: ControlSize,
    enabled: Vec<bool>,
}

/// Lays out `segs` left to right as one bar `h` tall with `outer` corners, drawn in `style` at
/// `size` (which picks the focus style; the bar's own corners shape the segments).
pub fn segment_bar(ui: &mut Ui, size: ControlSize, h: f32, outer: u8, style: BarStyle, segs: &[Seg]) -> Bar {
    let border = if matches!(style, BarStyle::Framed { .. }) { BORDER_W } else { 0.0 };
    let w = segs.iter().map(|s| s.w).sum::<f32>() + 2.0 * border;
    let (rect, bar) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let mut x = rect.left() + border;
    let responses = segs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let r = Rect::from_min_size(pos2(x, rect.top() + border), vec2(s.w, h - 2.0 * border));
            x += s.w;
            let sense = if s.enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
            ui.interact(r, segment_id(bar.id, i), sense)
        })
        .collect();
    Bar { responses, rect, outer, style, size, enabled: segs.iter().map(|s| s.enabled).collect() }
}

/// Segment `i`'s id, from the bar's: stable while a segment's content changes.
fn segment_id(bar: Id, i: usize) -> Id {
    bar.with(("segment", i))
}

impl Bar {
    /// The corners of segment `i`.
    fn corners(&self, i: usize) -> CornerRadius {
        let outer = match self.style {
            BarStyle::Framed { .. } => radius::inside(self.outer, BORDER_W as u8),
            BarStyle::Quiet { .. } => self.outer,
        };
        radius::segment(outer, i == 0, i + 1 == self.responses.len())
    }

    /// Paints the bar: its container, each segment's state (`latched[i]` keeps segment `i`
    /// latched, an open menu's), the dividers, and the border.
    pub fn paint(&self, ui: &Ui, latched: &[bool]) {
        let p = ui.painter();
        let single = self.responses.len() == 1;
        let border = Stroke::new(BORDER_W, color::border());
        if let BarStyle::Framed { fill } = self.style {
            p.rect_filled(self.rect, self.outer, fill);
        }
        for (i, r) in self.responses.iter().enumerate() {
            let on = self.enabled[i];
            let press = if on {
                Press::of(r.hovered(), r.is_pointer_button_down_on(), latched.get(i).copied().unwrap_or(false))
            } else {
                Press::Rest
            };
            // A framed bar of one segment is a single framed control: the target is the whole
            // frame, and it paints the border itself.
            let target = match self.style {
                BarStyle::Framed { .. } if single => {
                    Target { rect: self.rect, corners: CornerRadius::same(self.outer), edge: Some(border) }
                }
                _ => Target { rect: r.rect, corners: self.corners(i), edge: None },
            };
            paint_target(p, target, self.size, press, r.has_focus());
        }
        for r in &self.responses[..self.responses.len().saturating_sub(1)] {
            let x = r.rect.right().round() + 0.5;
            let (top, bottom) = match self.style {
                BarStyle::Framed { .. } => (self.rect.top() + DIVIDER_INSET, self.rect.bottom() - DIVIDER_INSET),
                BarStyle::Quiet { divider } => (r.rect.center().y - divider / 2.0, r.rect.center().y + divider / 2.0),
            };
            p.line_segment([pos2(x, top), pos2(x, bottom)], border);
        }
        if matches!(self.style, BarStyle::Framed { .. }) && !single {
            p.rect_stroke(self.rect, self.outer, border, StrokeKind::Inside);
        }
    }
}

#[cfg(test)]
#[path = "segment_bar_tests.rs"]
mod tests;
