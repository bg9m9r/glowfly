//! How a repository tab looks (`tabs.rs` lays the strip out): a shape with rounded top
//! corners, no fill while inactive (the chrome shows through), the hover fill in the same
//! shape, and the active tab lifted onto the panel colour with an accent line along its top.
//! Neighbouring quiet tabs are parted by a short line, and a worktree group's tabs share a
//! coloured strip along their bottom.

use super::theme::color;
use super::widgets::tab_glyph::{self, TabGlyph};
use super::widgets::{Cut, paint_cut_text};
use eframe::egui::{self, Color32, FontId, Rect, Stroke, pos2, vec2};

/// A tab's corners: rounded at the top, square where it meets the bar's bottom.
pub(super) const ROUNDING: egui::CornerRadius = egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 };

/// What a tab shows this frame.
#[derive(Clone, Copy, Default)]
pub(super) struct TabLook {
    pub active: bool,
    pub hovered: bool,
    pub show_close: bool,
    pub close_hovered: bool,
}

/// Paints one tab; returns where its name went and whether it was cut.
pub(super) fn paint_tab(p: &egui::Painter, rect: Rect, name: &str, font: FontId, look: TabLook) -> Cut {
    let close_rect = close_rect(rect);
    if look.active {
        p.rect_filled(rect, ROUNDING, color::panel());
        p.line_segment([rect.left_top() + vec2(6.0, 1.0), rect.right_top() + vec2(-6.0, 1.0)], Stroke::new(2.0_f32, color::accent()));
    } else if look.hovered {
        p.rect_filled(rect, ROUNDING, color::hover());
    }
    let text_col = if look.active { color::text() } else { color::weak() };
    // The name, cut with an ellipsis before the close button.
    let left = rect.left() + 12.0;
    let cut = paint_cut_text(p, pos2(left, rect.center().y), name, font, text_col, close_rect.left() - 4.0 - left);
    // Close button: shown on the active and hovered tabs.
    if look.show_close {
        tab_glyph::paint(p, close_rect.center(), TabGlyph::Close, text_col, look.close_hovered.then(color::selected));
    }
    cut
}

/// The short line between two tabs that are neither active nor hovered, `gap` wide after `left`.
pub(super) fn paint_separator(p: &egui::Painter, left: Rect, gap: f32) {
    let x = left.right() + gap / 2.0;
    let y = left.center().y;
    p.line_segment([pos2(x, y - 8.0), pos2(x, y + 8.0)], Stroke::new(1.0_f32, color::border()));
}

/// The color of the `n`th group from the left, so neighbouring groups differ.
pub(super) fn group_color(n: usize) -> Color32 {
    color::repo_group(n).gamma_multiply(0.8)
}

/// The strip under a grouped tab: inset at the group's outer ends, and across the `gap` to the
/// next tab elsewhere, so it runs unbroken along the group.
pub(super) fn paint_group_strip(p: &egui::Painter, rect: Rect, color: Color32, first: bool, last: bool, gap: f32) {
    let left = rect.left() + if first { 4.0 } else { 0.0 };
    let right = rect.right() + if last { -4.0 } else { gap };
    p.rect_filled(Rect::from_min_max(pos2(left, rect.bottom() - 3.0), pos2(right, rect.bottom() - 1.0)), 1.0, color);
}

/// Where a tab's close × sits.
pub(super) fn close_rect(tab: Rect) -> Rect {
    Rect::from_center_size(pos2(tab.right() - 14.0, tab.center().y), vec2(tab_glyph::BOX, tab_glyph::BOX))
}
