//! The tab bars' painted close × and new-tab +, drawn alike (one stroke, one hover box) so the
//! two read as one family: in the repository tab bar (`ui/tabs.rs`) and the terminal panel's
//! tab strip (`ui/terminal/tabs.rs`). The fonts lack both glyphs, so they are painted.

use eframe::egui::{self, Color32, CornerRadius, Id, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, vec2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TabGlyph {
    Close,
    Plus,
}

/// The hover box around a glyph (the repository tab's close button is this size).
pub const BOX: f32 = 16.0;
/// The click target of a standalone glyph button (the terminal strip).
pub const HIT: f32 = 20.0;
pub const STROKE: f32 = 1.4;
const CORNER: u8 = 3;
/// The × diagonals run ±4 on both axes.
const CLOSE_ARM: f32 = 4.0;
/// The + arms run ±4.5, about the ×'s weight of line.
const PLUS_ARM: f32 = 4.5;

/// The two strokes of `glyph` centred on `c`.
pub fn segments(glyph: TabGlyph, c: Pos2) -> [[Pos2; 2]; 2] {
    match glyph {
        TabGlyph::Close => {
            let a = CLOSE_ARM;
            [[c + vec2(-a, -a), c + vec2(a, a)], [c + vec2(-a, a), c + vec2(a, -a)]]
        }
        TabGlyph::Plus => {
            let a = PLUS_ARM;
            [[c + vec2(-a, 0.0), c + vec2(a, 0.0)], [c + vec2(0.0, -a), c + vec2(0.0, a)]]
        }
    }
}

/// Paints the hover box (when `hover_fill` is Some), then the glyph in `col`.
pub fn paint(p: &Painter, c: Pos2, glyph: TabGlyph, col: Color32, hover_fill: Option<Color32>) {
    if let Some(fill) = hover_fill {
        p.rect_filled(Rect::from_center_size(c, vec2(BOX, BOX)), CornerRadius::same(CORNER), fill);
    }
    let s = Stroke::new(STROKE, col);
    for seg in segments(glyph, c) {
        p.line_segment(seg, s);
    }
}

/// A glyph button whose click target is HIT x HIT around `centre`, with the id `id`. It is
/// labelled `name` for screen readers and tests, shows `name` on hover with a pointing hand,
/// and paints its box in `hover_fill` while hovered.
pub fn button_at(ui: &Ui, id: Id, centre: Pos2, glyph: TabGlyph, name: &str, col: Color32, hover_fill: Color32) -> Response {
    let resp = ui.interact(Rect::from_center_size(centre, vec2(HIT, HIT)), id, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    paint(ui.painter(), centre, glyph, col, resp.hovered().then_some(hover_fill));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    #[test]
    fn glyphs_fit_their_box() {
        let c = pos2(100.0, 50.0);
        let [a, b] = segments(TabGlyph::Close, c);
        assert_eq!(a, [c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)]);
        assert_eq!(b, [c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)]);
        let [h, v] = segments(TabGlyph::Plus, c);
        assert_eq!((h[0].y, h[1].y, h[0].x, h[1].x), (c.y, c.y, c.x - 4.5, c.x + 4.5), "horizontal arm");
        assert_eq!((v[0].x, v[1].x, v[0].y, v[1].y), (c.x, c.x, c.y - 4.5, c.y + 4.5), "vertical arm");
        let room = Rect::from_center_size(c, vec2(BOX, BOX)).shrink(STROKE / 2.0);
        for glyph in [TabGlyph::Close, TabGlyph::Plus] {
            assert!(segments(glyph, c).iter().flatten().all(|p| room.contains(*p)), "{glyph:?}");
        }
    }
}
