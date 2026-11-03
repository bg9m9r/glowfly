//! A small binary file's hex view (#471): a muted caption with both sizes and how many bytes
//! differ, then rows of a 4-digit hex offset, a `-`/`+` sign on changed rows, the hex bytes and
//! an ASCII column (printable bytes as themselves, any other as a muted dot). Bytes compare by
//! position (`glowfly_core::hex_rows`). A changed row is filled like a removed or added line and
//! each differing byte (hex and ASCII cell) like a changed word; a row on one side only takes
//! the line fill alone. Inline shows 16 bytes a row, a changed row's old row above its new one;
//! split shows 8 a side, old left and new right, aligned by offset. In the hunk view unchanged
//! rows more than two from a change fold into an "N unchanged rows" row; the whole-file view
//! shows every row. Read-only: it scrolls, and nothing in it is picked or copied.

use super::style::DiffStyle;
use crate::state::AppState;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use glowfly_core::hex_rows::{HexKind, HexRow, ascii, byte, differing, hex_rows};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::pointer::size_text;
use glowfly_core::repo::BinarySides;
use glowfly_core::settings::{DiffLayout, DiffMode};
use std::sync::Arc;

/// Unchanged rows kept on each side of a change in the hunk view.
const CONTEXT: usize = 2;
/// The caption row's height.
const CAPTION_H: f32 = 28.0;

/// What a drawn line shows: a fold, or a row's bytes from the old side, the new side or both.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Line {
    Fold(usize),
    Row { offset: usize, kind: HexKind, side: Shown },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shown {
    Old,
    New,
    /// Unchanged (inline), or both halves (split).
    Both,
}

/// The drawn lines, made from the sides on screen for a layout and mode and kept until they
/// change.
#[derive(Default)]
pub struct HexLines {
    key: Option<(Arc<BinarySides>, DiffLayout, DiffMode)>,
    lines: Vec<Line>,
    differing: usize,
}

impl HexLines {
    fn sync(&mut self, sides: &Arc<BinarySides>, layout: DiffLayout, mode: DiffMode) {
        if self.key.as_ref().is_some_and(|(k, l, m)| Arc::ptr_eq(k, sides) && *l == layout && *m == mode) {
            return;
        }
        let (old, new) = sides.hex().unwrap_or_default();
        let context = (mode == DiffMode::Hunks).then_some(CONTEXT);
        let rows = hex_rows(old, new, per_row(layout), context);
        self.lines = lines(&rows, layout);
        self.differing = differing(old, new);
        self.key = Some((sides.clone(), layout, mode));
    }
}

fn per_row(layout: DiffLayout) -> usize {
    match layout {
        DiffLayout::Inline => 16,
        DiffLayout::Split => 8,
    }
}

/// The rows as drawn lines: inline splits a changed row into its old and new rows.
fn lines(rows: &[HexRow], layout: DiffLayout) -> Vec<Line> {
    let mut out = Vec::with_capacity(rows.len() * 2);
    for &row in rows {
        match row {
            HexRow::Fold { count } => out.push(Line::Fold(count)),
            HexRow::Bytes { offset, kind } => {
                let row = |side| Line::Row { offset, kind, side };
                match (layout, kind) {
                    (DiffLayout::Inline, HexKind::Changed) => out.extend([row(Shown::Old), row(Shown::New)]),
                    (DiffLayout::Inline, HexKind::OldOnly) => out.push(row(Shown::Old)),
                    (DiffLayout::Inline, HexKind::NewOnly) => out.push(row(Shown::New)),
                    _ => out.push(row(Shown::Both)),
                }
            }
        }
    }
    out
}

/// The caption: both sizes and how many bytes differ, or what happened to the file.
fn caption(t: Lang, sides: &BinarySides, differ: usize) -> String {
    match (&sides.old, &sides.new) {
        (Some(a), Some(b)) => {
            let (old, new) = (size_text(a.size), size_text(b.size));
            t.trn("diff.hex.caption", differ, &[("old", &old), ("new", &new), ("count", &differ)])
        }
        (None, Some(b)) => t.trf("diff.hex.caption.added", &[("size", &size_text(b.size))]),
        (Some(a), None) => t.trf("diff.hex.caption.deleted", &[("size", &size_text(a.size))]),
        (None, None) => String::new(),
    }
}

/// Column positions of one side, relative to its left edge.
#[derive(Clone, Copy)]
struct Cols {
    ch: f32,
    offset: f32,
    sign: f32,
    hex: f32,
    ascii: f32,
    per_row: usize,
    /// The extra space after the 8th byte (inline only).
    group: f32,
    width: f32,
}

impl Cols {
    fn new(ch: f32, k: f32, per_row: usize) -> Self {
        let offset = 8.0 * k;
        let sign = offset + 4.0 * ch + 12.0 * k;
        let hex = sign + ch + 8.0 * k;
        let group = if per_row > 8 { ch } else { 0.0 };
        let ascii = hex + (per_row * 3 - 1) as f32 * ch + group + 16.0 * k;
        Self { ch, offset, sign, hex, ascii, per_row, group, width: ascii + per_row as f32 * ch + 8.0 * k }
    }

    fn hex_x(&self, j: usize) -> f32 {
        self.hex + (j * 3) as f32 * self.ch + if j >= 8 { self.group } else { 0.0 }
    }
}

/// Draws the hex view of `sides` (which have their bytes) below a caption.
pub(super) fn show(ui: &mut Ui, st: &AppState, sides: &Arc<BinarySides>, cache: &mut HexLines) {
    let layout = st.settings.diff_layout;
    cache.sync(sides, layout, st.settings.diff_mode);
    let t = st.settings.lang();
    let style = DiffStyle::new(&st.settings);
    let (cap_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), CAPTION_H), Sense::hover());
    let body = FontId::proportional(type_scale::BODY);
    let text = caption(t, sides, cache.differing);
    ui.painter().text(pos2(cap_rect.left() + 8.0, cap_rect.center().y), Align2::LEFT_CENTER, text, body, color::weak());

    // A glyph's advance as laid out in a run (which can differ from `glyph_width` by rounding),
    // so the cell fills and the ASCII column line up with the painted strings.
    let ch = ui.fonts(|f| f.layout_no_wrap("0".repeat(32), style.font.clone(), Color32::WHITE).size().x / 32.0);
    let cols = Cols::new(ch, style.k, per_row(layout));
    let (old, new) = sides.hex().unwrap_or_default();
    let halves = if layout == DiffLayout::Split { 2.0 } else { 1.0 };
    let lines = &cache.lines;
    egui::ScrollArea::both().id_salt("diff-hex").auto_shrink([false, false]).show_rows(ui, style.row_h, lines.len(), |ui, range| {
        let width = ui.available_width().max(cols.width * halves);
        for i in range {
            let (rect, _) = ui.allocate_exact_size(vec2(width, style.row_h), Sense::hover());
            let p = Painter { ui, style: &style, cols, old, new };
            match lines[i] {
                Line::Fold(count) => p.fold(rect, &t.trn("diff.hex.fold", count, &[("count", &count)])),
                Line::Row { offset, kind, side: Shown::Both } if layout == DiffLayout::Split => {
                    let half = (width / 2.0).max(cols.width);
                    let left = Rect::from_min_size(rect.min, vec2(half, rect.height()));
                    let right = Rect::from_min_size(pos2(left.right(), rect.top()), vec2(half, rect.height()));
                    p.half(left, offset, kind, Shown::Old);
                    p.half(right, offset, kind, Shown::New);
                    ui.painter().vline(left.right(), rect.y_range(), Stroke::new(1.0_f32, color::border()));
                }
                Line::Row { offset, kind, side } => p.half(rect, offset, kind, side),
            }
        }
    });
}

struct Painter<'a> {
    ui: &'a Ui,
    style: &'a DiffStyle,
    cols: Cols,
    old: Option<&'a [u8]>,
    new: Option<&'a [u8]>,
}

impl Painter<'_> {
    fn fold(&self, rect: Rect, text: &str) {
        let x = rect.left() + self.cols.hex;
        self.ui.painter().text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, self.style.font.clone(), color::weak());
    }

    /// One side's row in `rect`: the old side's bytes for `Old` (and an unchanged `Both`), the
    /// new side's for `New`. A side that doesn't reach this row leaves it empty.
    fn half(&self, rect: Rect, offset: usize, kind: HexKind, side: Shown) {
        let (own, other) = if side == Shown::New { (self.new, self.old) } else { (self.old, self.new) };
        if own.is_none_or(|b| b.len() <= offset) {
            return;
        }
        let p = self.ui.painter();
        let (line_fill, word_fill, sign) = match side {
            Shown::Old => (color::diff_del(), color::diff_del_word(), "-"),
            Shown::New => (color::diff_add(), color::diff_add_word(), "+"),
            Shown::Both => (Color32::TRANSPARENT, Color32::TRANSPARENT, ""),
        };
        let changed = kind != HexKind::Same;
        if changed {
            p.rect_filled(rect, CornerRadius::ZERO, line_fill);
        }
        let (c, font, y) = (self.cols, &self.style.font, rect.center().y);
        let at = |x: f32| pos2(rect.left() + x, y);
        p.text(at(c.offset), Align2::LEFT_CENTER, format!("{offset:04x}"), font.clone(), color::weak());
        if changed {
            p.text(at(c.sign), Align2::LEFT_CENTER, sign, font.clone(), color::text());
        }
        // The bytes paint as one hex string and one ASCII string a row, over the changed bytes'
        // cell fills.
        let cell_h = self.style.row_h - 4.0;
        let (mut hex, mut text) = (String::new(), LayoutJob::default());
        for j in 0..c.per_row {
            let i = offset + j;
            let Some(b) = byte(own, i) else { break };
            if kind == HexKind::Changed && byte(other, i) != Some(b) {
                let top = y - cell_h / 2.0;
                let cell = Rect::from_min_size(pos2(rect.left() + c.hex_x(j) - 1.0, top), vec2(2.0 * c.ch + 2.0, cell_h));
                p.rect_filled(cell, CornerRadius::same(2), word_fill);
                let glyph = Rect::from_min_size(pos2(rect.left() + c.ascii + j as f32 * c.ch, top), vec2(c.ch, cell_h));
                p.rect_filled(glyph, CornerRadius::same(2), word_fill);
            }
            let gap = match j {
                0 => "",
                8 if c.group > 0.0 => "  ",
                _ => " ",
            };
            hex.push_str(&format!("{gap}{b:02x}"));
            let (glyph, colour) = ascii(b).map_or(('.', color::weak()), |g| (g, color::text()));
            text.append(&glyph.to_string(), 0.0, TextFormat::simple(font.clone(), colour));
        }
        p.text(at(c.hex), Align2::LEFT_CENTER, hex, font.clone(), color::text());
        let galley = p.layout_job(text);
        p.galley(pos2(rect.left() + c.ascii, y - galley.size().y / 2.0), galley, color::text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_splits_a_changed_row_and_split_keeps_it_whole() {
        let rows = [
            HexRow::Fold { count: 3 },
            HexRow::Bytes { offset: 48, kind: HexKind::Same },
            HexRow::Bytes { offset: 64, kind: HexKind::Changed },
            HexRow::Bytes { offset: 80, kind: HexKind::NewOnly },
        ];
        let inline = lines(&rows, DiffLayout::Inline);
        let sides: Vec<_> = inline.iter().map(|l| if let Line::Row { side, .. } = l { Some(*side) } else { None }).collect();
        assert_eq!(sides, [None, Some(Shown::Both), Some(Shown::Old), Some(Shown::New), Some(Shown::New)]);
        assert_eq!(lines(&rows, DiffLayout::Split).len(), 4, "split shows each row once, both halves");
    }

    #[test]
    fn columns_leave_room_for_every_byte() {
        let c = Cols::new(7.5, 1.0, 16);
        assert_eq!(c.hex_x(8) - c.hex_x(7), 3.0 * 7.5 + 7.5, "a gap after the 8th byte");
        assert!(c.ascii >= c.hex_x(15) + 2.0 * 7.5 + 16.0);
        let s = Cols::new(7.5, 1.0, 8);
        assert!(s.width < c.width && s.group == 0.0);
    }
}
