//! Cells of the conflict view's panes and output: a line number and text on a tint, clipped to
//! the column, the text broken into visual lines when long lines wrap, and the row heights
//! that makes.

use super::diff::wrap::Wrap;
use super::diff::{ROW_H, mono};
use super::theme::color;
use eframe::egui::{Align2, Color32, Rect, Ui, pos2, vec2};
use glowfly_core::repo::{Segment, Side};

/// Line numbers end here; text starts at `TEXT_X` (both after a cell's inset).
pub(super) const GUTTER: f32 = 40.0;
pub(super) const TEXT_X: f32 = 50.0;
/// The panes' inset for the line checkboxes, on every row so numbers line up.
pub(super) const CHECK_W: f32 = 18.0;

/// The narrowest a wrapped line gets, in columns.
const MIN_COLS: usize = 8;
/// Room kept free after the text so wrapped text clears the edge.
const PAD: f32 = 8.0;

/// Trims the terminator and shows tabs as 4 spaces (the file is written unchanged).
pub(super) fn display(line: &str) -> String {
    line.trim_end_matches('\n').trim_end_matches('\r').replace('\t', "    ")
}

pub(super) fn tint(side: Side) -> Color32 {
    match side {
        Side::Ours => color::wash(color::info(), 0.18),
        Side::Theirs => color::wash(color::progress(), 0.18),
    }
}

/// One cell: background `bg`, line number and text in `fg` (`inset` from its left edge),
/// clipped to its column. The number and the first visual line sit in the row's first `ROW_H`;
/// with `wrap`, the text goes on below it a line at a time.
pub(super) fn cell(ui: &Ui, rect: Rect, inset: f32, number: Option<u32>, text: &str, (bg, fg): (Color32, Color32), wrap: Option<Wrap>) {
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, 0.0, bg);
    let (y, left) = (rect.top() + ROW_H / 2.0, rect.left() + inset);
    if let Some(n) = number {
        p.text(pos2(left + GUTTER, y), Align2::RIGHT_CENTER, n.to_string(), mono(), color::faint());
    }
    match wrap {
        Some(w) => {
            for (i, r) in glowfly_core::wrap::segments(text, w.cols).enumerate() {
                p.text(pos2(left + TEXT_X, y + i as f32 * w.line_h), Align2::LEFT_CENTER, &text[r], mono(), fg);
            }
        }
        None => {
            p.text(pos2(left + TEXT_X, y), Align2::LEFT_CENTER, text, mono(), fg);
        }
    }
}

/// How many visual lines `text` (as `display` shows it) takes with `wrap`.
pub(super) fn visual_lines(text: &str, wrap: Option<Wrap>) -> usize {
    wrap.map_or(1, |w| glowfly_core::wrap::segments(text, w.cols).count().max(1))
}

/// The accent bar marking the current chunk.
pub(super) fn current_bar(ui: &Ui, rect: Rect) {
    let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).rect_filled(bar, 0.0, color::accent());
}

/// Width for rows of `columns` columns, each with `inset`: the panel, or wider when a line
/// needs it. Wrapped (`wrap`), always the panel: nothing scrolls sideways.
pub(super) fn content_width(ui: &Ui, widest: usize, columns: f32, inset: f32, wrap: bool) -> f32 {
    if wrap {
        return ui.available_width();
    }
    let char_w = ui.fonts(|f| f.glyph_width(&mono(), 'M'));
    ui.available_width().max(columns * (inset + TEXT_X + widest as f32 * char_w + 20.0))
}

/// Display width of the longest line, in characters.
pub(super) fn widest(segs: &[Segment]) -> usize {
    let w = |lines: &[String]| {
        let width = |l: &String| l.trim_end_matches(['\n', '\r']).chars().map(|c| if c == '\t' { 4 } else { 1 }).sum::<usize>();
        lines.iter().map(width).max().unwrap_or(0)
    };
    segs.iter()
        .map(|s| match s {
            Segment::Clean(l) => w(l),
            Segment::Conflict(c) => w(&c.ours).max(w(&c.theirs)),
        })
        .max()
        .unwrap_or(0)
}

/// How long lines break in rows of `columns` columns, each with `inset` before its number and
/// text, at the width `ui` offers less a scrollbar; `None` unless wrapping is `on`.
pub(super) fn conflict_wrap(ui: &Ui, on: bool, columns: f32, inset: f32) -> Option<Wrap> {
    if !on {
        return None;
    }
    let char_w = ui.fonts(|f| f.glyph_width(&mono(), 'M'));
    let line_h = ui.fonts(|f| f.row_height(&mono()));
    let scroll = &ui.spacing().scroll;
    let avail = ui.available_width() - scroll.allocated_width().max(scroll.bar_width);
    let text_w = (avail / columns).floor() - inset - TEXT_X - PAD;
    let cols = if char_w > 0.0 { (text_w / char_w).floor().max(0.0) as usize } else { 0 };
    Some(Wrap { cols: cols.max(MIN_COLS), line_h })
}

/// The top of each of `n` rows and the total at the end, starting at 0: a row of `lines_of(i)`
/// visual lines is `ROW_H` plus `line_h` for each line after the first.
pub(super) fn offsets(n: usize, wrap: Wrap, lines_of: impl Fn(usize) -> usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n + 1);
    let mut y = 0.0;
    out.push(y);
    for i in 0..n {
        y += ROW_H + lines_of(i).saturating_sub(1) as f32 * wrap.line_h;
        out.push(y);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_grow_by_visual_lines() {
        let w = Wrap { cols: 10, line_h: 15.0 };
        let o = offsets(3, w, |i| [1, 3, 1][i]);
        assert_eq!(o, [0.0, ROW_H, 2.0 * ROW_H + 30.0, 3.0 * ROW_H + 30.0]);
        assert_eq!(visual_lines(&"x".repeat(25), Some(w)), 3);
        assert_eq!(visual_lines(&"x".repeat(25), None), 1);
        assert_eq!(visual_lines("", Some(w)), 1);
    }
}
