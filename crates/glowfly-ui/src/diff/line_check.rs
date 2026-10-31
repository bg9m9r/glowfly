//! How a changed line shows that it is selected, or can be: the bar on its left edge, and in
//! the sign column a check box that stands in for the `+`/`-` while the gutter is hovered
//! (hollow) or the line is selected (filled, with a check mark).

use super::LineHover;
use crate::ui::theme::color;
use crate::ui::theme::contrast::ratio;
use crate::ui::theme::tokens::radius;
use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

/// The check box's side.
const BOX: f32 = 14.0;
/// The hollow box's line width.
const BOX_STROKE: f32 = 1.4;
/// The check mark's line width.
const CHECK_STROKE: f32 = 1.8;
/// The selected line's bar width.
const BAR: f32 = 3.0;

/// The bar on a line's left edge: the accent when selected, faint when hovered.
pub(crate) fn paint_mark(ui: &Ui, rect: Rect, selected: bool, hovered: bool) {
    let c = match (selected, hovered) {
        (true, _) => color::accent(),
        (false, true) => color::faint(),
        _ => return,
    };
    let bar = Rect::from_min_size(rect.min, vec2(BAR, rect.height()));
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).rect_filled(bar, 0.0, c);
}

/// The sign column of a line whose sign glyph starts at `at` (left, vertical centre): the glyph
/// `sign` in `fg`, or, on a selected line or one whose gutter is hovered, the check box in its
/// place.
pub(crate) fn gutter_sign(p: &Painter, at: Pos2, sign: char, fg: Color32, font: &FontId, selected: bool, hover: LineHover) {
    if !selected && !hover.gutter {
        p.text(at, Align2::LEFT_CENTER, sign, font.clone(), fg);
        return;
    }
    let glyph_w = p.ctx().fonts(|f| f.glyph_width(font, '+'));
    // Centred on where the glyph would be, on whole pixels so the stroke stays crisp.
    let c = pos2((at.x + glyph_w / 2.0).round(), at.y.round());
    check_box(p, c, selected);
}

/// A rounded box `BOX` across centred on `c`: filled with the accent and checked when
/// `selected`, otherwise hollow in the weak colour.
fn check_box(p: &Painter, c: Pos2, selected: bool) {
    let r = Rect::from_center_size(c, vec2(BOX, BOX));
    let round = CornerRadius::same(radius::SM);
    if !selected {
        p.rect_stroke(r, round, Stroke::new(BOX_STROKE, color::weak()), StrokeKind::Inside);
        return;
    }
    let accent = color::accent();
    p.rect_filled(r, round, accent);
    // Black or white, whichever stands out more on the accent.
    let ink = if ratio(Color32::WHITE, accent) >= ratio(Color32::BLACK, accent) { Color32::WHITE } else { Color32::BLACK };
    let s = Stroke::new(CHECK_STROKE, ink);
    let (a, b, d) = (pos2(c.x - 3.6, c.y + 0.2), pos2(c.x - 1.0, c.y + 2.9), pos2(c.x + 3.8, c.y - 2.8));
    p.add(Shape::line(vec![a, b, d], s));
}
