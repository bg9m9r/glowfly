//! The tab bars' painted close × and new-tab +, drawn alike (one stroke, one hover box) so the
//! two read as one family: in the repository tab bar (`ui/tabs.rs`) and the terminal panel's
//! tab strip (`ui/terminal/tabs.rs`). The fonts lack both glyphs, so they are painted, centred
//! on the pixel grid (`icon_snap`) so neither looks off-centre in its box. Each takes clicks on
//! at least the least hit area (`HIT`), more than the box it paints, and rings its box while it
//! has the keyboard focus.

use super::button_look::{self, Press};
use super::icon_snap;
use super::quiet_target::State;
use crate::ui::theme::tokens::{self, HIT_MIN};
use eframe::egui::{self, Color32, CornerRadius, Id, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, vec2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TabGlyph {
    Close,
    Plus,
}

/// The hover box around a glyph (the repository tab's close button is this size).
pub const BOX: f32 = 16.0;
/// The click target of a glyph button: the least hit area.
pub const HIT: f32 = HIT_MIN;
pub const STROKE: f32 = 1.4;
const CORNER: u8 = 3;
/// The × diagonals run ±4 on both axes.
const CLOSE_ARM: f32 = 4.0;
/// The + arms run ±4.5, about the ×'s weight of line (their ends then land on whole pixels).
const PLUS_ARM: f32 = 4.5;

/// The two strokes of `glyph` centred on `c` at `ppp` physical pixels per point: the centre
/// moved onto the pixel grid for the line width, and the +'s ends on pixel edges.
pub fn segments(glyph: TabGlyph, c: Pos2, ppp: f32) -> [[Pos2; 2]; 2] {
    match glyph {
        TabGlyph::Close => {
            let c = icon_snap::snap_centre(c, STROKE, ppp);
            let a = CLOSE_ARM;
            [[c + vec2(-a, -a), c + vec2(a, a)], [c + vec2(-a, a), c + vec2(a, -a)]]
        }
        TabGlyph::Plus => icon_snap::plus_segments(c, PLUS_ARM, STROKE, ppp),
    }
}

/// The box `glyph` sits in around `c`: where its hover layer and focus ring go.
fn glyph_box(glyph: TabGlyph, c: Pos2, ppp: f32) -> Rect {
    let [h, v] = segments(glyph, c, ppp);
    let centre = match glyph {
        TabGlyph::Plus => egui::pos2(v[0].x, h[0].y),
        TabGlyph::Close => h[0].lerp(h[1], 0.5),
    };
    Rect::from_center_size(centre, vec2(BOX, BOX))
}

/// Paints the hover box (when `hover_fill` is Some), then the glyph in `col`.
pub fn paint(p: &Painter, c: Pos2, glyph: TabGlyph, col: Color32, hover_fill: Option<Color32>) {
    let ppp = p.ctx().pixels_per_point();
    if let Some(fill) = hover_fill {
        p.rect_filled(glyph_box(glyph, c, ppp), CornerRadius::same(CORNER), fill);
    }
    strokes(p, c, glyph, col, ppp);
}

/// Paints `glyph` as a quiet target in state `s`: the hover, pressed or latched layer on its box
/// (nothing at rest), the inset focus ring on the box while focused, then the glyph in `col`.
pub fn paint_quiet(p: &Painter, c: Pos2, glyph: TabGlyph, col: Color32, s: State) {
    let ppp = p.ctx().pixels_per_point();
    let rect = glyph_box(glyph, c, ppp);
    button_look::paint_quiet(p, rect, CornerRadius::same(CORNER), Press::of(s.hovered, s.down, s.latched));
    if s.focused {
        tokens::focus_ring_inset(p, rect, CORNER);
    }
    strokes(p, c, glyph, col, ppp);
}

fn strokes(p: &Painter, c: Pos2, glyph: TabGlyph, col: Color32, ppp: f32) {
    let s = Stroke::new(STROKE, col);
    for seg in segments(glyph, c, ppp) {
        p.line_segment(seg, s);
    }
}

/// A glyph button whose click target is HIT x HIT around `centre`, with the id `id`. It is
/// labelled `name` for screen readers and tests, shows `name` on hover with a pointing hand,
/// paints its box in `hover_fill` while hovered and rings it while focused.
pub fn button_at(ui: &Ui, id: Id, centre: Pos2, glyph: TabGlyph, name: &str, col: Color32, hover_fill: Color32) -> Response {
    let resp = ui.interact(Rect::from_center_size(centre, vec2(HIT, HIT)), id, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    paint(ui.painter(), centre, glyph, col, resp.hovered().then_some(hover_fill));
    if resp.has_focus() {
        tokens::focus_ring_inset(ui.painter(), glyph_box(glyph, centre, ui.ctx().pixels_per_point()), CORNER);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    #[test]
    fn glyphs_fit_their_box() {
        let c = pos2(100.0, 50.0);
        let [a, b] = segments(TabGlyph::Close, c, 1.5);
        assert_eq!(a, [c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)]);
        assert_eq!(b, [c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)]);
        let [h, v] = segments(TabGlyph::Plus, c, 1.0);
        // 1.4 px covers one pixel at 100 %: the centre on a pixel's middle, the ends on edges.
        let m = pos2(100.5, 50.5);
        assert_eq!((h[0].y, h[1].y, h[0].x, h[1].x), (m.y, m.y, m.x - 4.5, m.x + 4.5), "horizontal arm");
        assert_eq!((v[0].x, v[1].x, v[0].y, v[1].y), (m.x, m.x, m.y - 4.5, m.y + 4.5), "vertical arm");
        for ppp in [1.0, 1.25, 1.5, 2.0] {
            for glyph in [TabGlyph::Close, TabGlyph::Plus] {
                let room = glyph_box(glyph, c, ppp).shrink(STROKE / 2.0);
                assert!(segments(glyph, c, ppp).iter().flatten().all(|p| room.contains(*p)), "{glyph:?} at {ppp}");
            }
        }
    }
}
