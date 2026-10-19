//! The sidebar's list rows: one line of text with optional dim text (words, or counts with
//! painted arrows) and a marker slot at the right, plus the skip-if-scrolled-away helper the long
//! sections use.

use super::theme::color;

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

/// One piece of a list row's dim right-hand text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part<'a> {
    /// Words, drawn as text.
    Text(&'a str),
    /// Commits ahead of the upstream: the count, then a painted up arrow.
    Ahead(usize),
    /// Commits behind the upstream: the count, then a painted down arrow.
    Behind(usize),
}

/// One sidebar list row. Returns the click response; `right` is optional dim text at the right.
pub fn list_row(ui: &mut Ui, indent: f32, text: &str, right: Option<(&str, Color32)>, selected: bool, strong: bool) -> Response {
    let parts = right.map(|(r, _)| [Part::Text(r)]);
    let right = right.zip(parts.as_ref()).map(|((_, c), p)| (&p[..], c));
    list_row_slot(ui, indent, text, right, 0.0, selected, strong).0
}

/// `list_row` with `slot` px kept free at the right edge (right of the dim right-hand parts), for
/// a painted marker; returns the row's response and the slot's rect (zero-width when slot is 0).
pub fn list_row_slot(
    ui: &mut Ui,
    indent: f32,
    text: &str,
    right: Option<(&[Part<'_>], Color32)>,
    slot: f32,
    selected: bool,
    strong: bool,
) -> (Response, Rect) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests).
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, 0.0, color::selected());
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    let mut right_w = if slot > 0.0 { slot + 14.0 } else { 0.0 };
    if let Some((parts, col)) = right {
        let (total, pieces) = layout_parts(&p, parts, col);
        right_w = total + 14.0 + slot;
        let mut x = rect.right() - 10.0 - slot - total;
        let cy = rect.center().y;
        for (i, piece) in pieces.into_iter().enumerate() {
            if i > 0 {
                x += GAP;
            }
            let w = piece.galley.size().x;
            p.galley(pos2(x, cy - piece.galley.size().y / 2.0), piece.galley, col);
            x += w;
            if let Some(up) = piece.arrow {
                paint_arrow(&p, pos2(x + ARROW_GAP + ARROW_W / 2.0, cy), up, col);
                x += ARROW_GAP + ARROW_W;
            }
        }
    }
    let text_rect = Rect::from_min_max(pos2(rect.left() + indent, rect.top()), pos2(rect.right() - right_w, rect.bottom()));
    let font = if strong { FontId::new(13.0, egui::FontFamily::Proportional) } else { FontId::proportional(13.0) };
    let col = if strong { color::strong() } else { color::text() };
    let g = p.layout_no_wrap(text.to_owned(), font, col);
    let clipped = p.with_clip_rect(text_rect);
    clipped.galley(pos2(text_rect.left(), rect.center().y - g.size().y / 2.0), g, col);
    let marker = Rect::from_min_max(pos2(rect.right() - 10.0 - slot, rect.top()), pos2(rect.right() - 10.0, rect.bottom()));
    (resp, marker)
}

/// The space between two right-hand parts (a space's width at 12 px).
const GAP: f32 = 6.0;
/// The space between a count and its arrow, and the arrow's width.
const ARROW_GAP: f32 = 1.5;
const ARROW_W: f32 = 6.0;

/// A laid-out right-hand part: its text, and for a count, which way its arrow points.
struct Piece {
    galley: Arc<Galley>,
    arrow: Option<bool>,
}

/// Lays out `parts` in the 12 px dim font; returns their total width (gaps included) and pieces.
fn layout_parts(p: &Painter, parts: &[Part<'_>], col: Color32) -> (f32, Vec<Piece>) {
    let font = FontId::proportional(12.0);
    let pieces: Vec<Piece> = parts
        .iter()
        .map(|part| {
            let (text, arrow) = match *part {
                Part::Text(t) => (t.to_owned(), None),
                Part::Ahead(n) => (n.to_string(), Some(true)),
                Part::Behind(n) => (n.to_string(), Some(false)),
            };
            Piece { galley: p.layout_no_wrap(text, font.clone(), col), arrow }
        })
        .collect();
    let widths: f32 = pieces.iter().map(|pc| pc.galley.size().x + if pc.arrow.is_some() { ARROW_GAP + ARROW_W } else { 0.0 }).sum();
    let gaps = GAP * pieces.len().saturating_sub(1) as f32;
    (widths + gaps, pieces)
}

/// A small arrow centred on `c`, pointing up (ahead) or down (behind). Painted because the
/// bundled fonts have no arrow glyphs.
fn paint_arrow(p: &Painter, c: Pos2, up: bool, col: Color32) {
    let s = Stroke::new(1.3_f32, col);
    // The down arrow sits half a pixel higher: its tip's round caps reach lower than a stem end.
    let (tip, tail) = if up { (c.y - 3.5, c.y + 3.5) } else { (c.y + 3.0, c.y - 4.0) };
    let back = if up { 2.6 } else { -2.6 };
    p.line_segment([pos2(c.x, tail), pos2(c.x, tip)], s);
    p.line_segment([pos2(c.x - 2.6, tip + back), pos2(c.x, tip)], s);
    p.line_segment([pos2(c.x + 2.6, tip + back), pos2(c.x, tip)], s);
}

/// The height of a `list_row`.
pub const LIST_ROW_H: f32 = 24.0;

/// For a `list_row` about to be drawn: when it would be scrolled out of view, takes its space
/// (and one widget id, as the row does) and returns true, so the caller skips it. Long sections
/// of the left panel then cost little more than their visible rows.
pub fn list_row_hidden(ui: &mut Ui) -> bool {
    let size = vec2(ui.available_width(), LIST_ROW_H);
    if ui.is_rect_visible(Rect::from_min_size(ui.cursor().min, size)) {
        return false;
    }
    ui.allocate_space(size);
    true
}
