//! The selected pair's pane. A changed pair shows its two versions ("#n sha summary") over the
//! interdiff, drawn with the diff viewer's look (`DiffStyle`'s font and row height, no line
//! numbers): outer hunk headers on the hunk fill, outer `+`/`-` lines on the added and removed
//! fills with the outer sign in the gutter, and the inner diff's own marker in the added or
//! removed colour. Rows are laid out only where visible and scroll sideways for long lines. A
//! pair that is the same, dropped or new gets a sentence instead.

use crate::state::AppState;
use crate::state::journal::short;
use crate::ui::common::{added_color, removed_color};
use crate::ui::diff::wrap::{self, RowMetrics};
use crate::ui::diff::{DiffStyle, LineHover, gutter_sign, paint_header, paint_line};
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::paint_cut_text;
use eframe::egui::{self, Align2, Color32, RichText, Sense, Ui, pos2, vec2};
use trellis_core::diff_rows::{DiffRow, inline_rows};
use trellis_core::i18n::Lang;
use trellis_core::range_diff::{Pair, Side, Status};
use trellis_core::repo::FileDiff;

const INSET: f32 = 16.0;
const SIDE_ROW_H: f32 = 22.0;

pub(super) fn show(ui: &mut Ui, st: &AppState) {
    let Some(view) = &st.range_diff else { return };
    let t = st.settings.lang();
    let Some(i) = view.selected else {
        return sentence(ui, t.tr("range_diff.pane.select"), color::weak());
    };
    let Some(pair) = view.pair(i) else { return };
    match (pair.status, &pair.interdiff) {
        (Status::Changed, Some(diff)) => {
            sides(ui, t, pair);
            rows(ui, st, diff, view.widest.get(i).copied().unwrap_or(0));
        }
        (Status::Changed | Status::Unchanged, _) => sentence(ui, t.tr("range_diff.pane.same"), color::text()),
        (Status::Removed, _) => sentence(ui, t.tr("range_diff.pane.removed"), color::text()),
        (Status::Added, _) => sentence(ui, t.tr("range_diff.pane.added"), color::text()),
    }
}

fn sentence(ui: &mut Ui, text: &str, col: Color32) {
    egui::Frame::new().inner_margin(egui::Margin::same(INSET as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text).color(col)).wrap());
    });
}

/// The old and new versions, one line each, above the rows.
fn sides(ui: &mut Ui, t: Lang, pair: &Pair) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 2.0 * SIDE_ROW_H + 12.0), Sense::hover());
    let p = ui.painter();
    let (mono, body) = (type_scale::mono(type_scale::SMALL), type_scale::font(type_scale::BODY));
    let args = |s: &Side| (s.index, short(s.oid));
    let old = pair.old.as_ref().map(|s| (s, args(s))).map(|(s, (n, sha))| (s, t.trf("range_diff.side.old", &[("n", &n), ("sha", &sha)])));
    let new = pair.new.as_ref().map(|s| (s, args(s))).map(|(s, (n, sha))| (s, t.trf("range_diff.side.new", &[("n", &n), ("sha", &sha)])));
    let line = |y: f32, side: Option<(&Side, String)>| {
        let Some((s, label)) = side else { return };
        let x = rect.left() + INSET;
        let at = p.text(pos2(x, y), Align2::LEFT_CENTER, label, mono.clone(), color::weak());
        let sx = at.right() + 8.0;
        paint_cut_text(p, pos2(sx, y), &s.summary, body.clone(), color::text(), (rect.right() - INSET - sx).max(0.0));
    };
    line(rect.top() + 6.0 + SIDE_ROW_H / 2.0, old);
    line(rect.top() + 6.0 + SIDE_ROW_H * 1.5, new);
    p.hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
}

/// The interdiff's rows; `widest` is its widest line in columns.
fn rows(ui: &mut Ui, st: &AppState, diff: &FileDiff, widest: usize) {
    let mut style = DiffStyle::new(&st.settings);
    style.numbers = false;
    let cols = style.inline_cols();
    let font = style.font.clone();
    let char_w = ui.fonts(|f| f.glyph_width(&font, 'M'));
    // An implicit hunk (lines before any outer @@) has no header row.
    let rows: Vec<DiffRow> =
        inline_rows(diff).into_iter().filter(|r| !matches!(r, DiffRow::Header(h) if diff.hunks[*h].header.is_empty())).collect();
    let width = ui.available_width().max(cols.text + widest as f32 * char_w + 40.0);
    wrap::scroll(ui, "range-diff-interdiff", rows.len(), RowMetrics::Uniform(style.row_h), None, |ui, i, h| {
        let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
        let p = ui.painter();
        let y = rect.top() + style.row_h / 2.0;
        match rows[i] {
            DiffRow::Header(hunk) => {
                p.rect_filled(rect, 0.0, color::hunk_bg());
                paint_header(p, pos2(rect.left() + cols.sign, y), &diff.hunks[hunk].header, None, &font, style.tab);
            }
            DiffRow::Line { hunk, line } => {
                let dl = &diff.hunks[hunk].lines[line];
                let (bg, sign) = match dl.origin {
                    '+' => (color::diff_add(), '+'),
                    '-' => (color::diff_del(), '-'),
                    _ => (Color32::TRANSPARENT, ' '),
                };
                p.rect_filled(rect, 0.0, bg);
                gutter_sign(p, pos2(rect.left() + cols.sign, y), sign, color::text(), &font, false, LineHover::default());
                let x = rect.left() + cols.text;
                let marker = dl.text.chars().next().filter(|c| matches!(c, '+' | '-'));
                match marker {
                    Some(m) => {
                        let ink = if m == '+' { added_color() } else { removed_color() };
                        p.text(pos2(x, y), Align2::LEFT_CENTER, m, font.clone(), ink);
                        let rest = &dl.text[1..];
                        paint_line(p, pos2(x + char_w, y), rest, &[], None, &font, color::text(), style.tab, None);
                    }
                    None => paint_line(p, pos2(x, y), &dl.text, &[], None, &font, color::text(), style.tab, None),
                }
            }
            DiffRow::Block(_) | DiffRow::Fold(_) | DiffRow::GapLine { .. } => {}
        }
    });
}
