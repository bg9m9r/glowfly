//! A repository's colour swatch: a small rounded square in its hue, painted on its tab, before
//! its path in the repository lists and on the Color submenu's rows. Square, so it never reads
//! as a branch's round dot.

use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, pos2, vec2};

/// The swatch's size in lists and menus.
pub const SIZE: f32 = 10.0;
/// The room a swatch takes before a label or path.
pub const ROOM: f32 = 16.0;

/// Paints a swatch `size` across in `c`, its left edge's middle at `left_centre`.
pub fn paint(p: &Painter, left_centre: Pos2, size: f32, c: Color32) {
    let rect = Rect::from_min_size(pos2(left_centre.x, left_centre.y - size / 2.0), vec2(size, size));
    p.rect_filled(rect, CornerRadius::same((size * 0.25).round() as u8), c);
}

/// Paints a swatch in `c` centred in a menu row's icon slot (`menu::item(..).icon`).
pub fn paint_slot(p: &Painter, slot: Rect, c: Color32) {
    paint(p, pos2(slot.center().x - SIZE / 2.0, slot.center().y), SIZE, c);
}
