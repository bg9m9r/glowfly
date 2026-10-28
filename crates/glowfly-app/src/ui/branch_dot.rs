//! A branch's colour dot: a small filled circle in its line tone, painted left of its name in the
//! sidebar's branch rows, the Branch picker's menu, the Color submenu and the commit details
//! header. In menus the dot goes in the row's icon column (`paint_slot`).

use eframe::egui::{Color32, Painter, Rect, pos2};

/// The room a dot takes before a name.
pub const DOT_ROOM: f32 = 14.0;
pub(crate) const DOT_R: f32 = 4.0;

/// Paints a dot in colour `c`, `indent` from `row`'s left edge, centred on the row.
pub fn paint(p: &Painter, row: Rect, indent: f32, c: Color32) {
    p.circle_filled(pos2(row.left() + indent + DOT_R, row.center().y), DOT_R, c);
}

/// Paints a dot in `c` centred in a menu row's icon slot (`menu::item(..).icon`).
pub fn paint_slot(p: &Painter, slot: Rect, c: Color32) {
    p.circle_filled(slot.center(), DOT_R, c);
}
