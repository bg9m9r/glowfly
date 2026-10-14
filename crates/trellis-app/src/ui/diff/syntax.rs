//! Painting diff line text with syntax colors. The spans come precomputed with the diff
//! (`trellis_core::highlight`); this only lays them out in the palette's token colors, after
//! expanding tabs to the tab stops.

use crate::ui::theme::color;
use eframe::egui::text::{CCursor, LayoutJob, TextFormat};
use eframe::egui::{Align2, Color32, FontId, Galley, Painter, Pos2, Rect, pos2};
use std::ops::Range;
use trellis_core::highlight::{Span, TokenKind};
use trellis_core::tabs::{self, Expanded};

/// `paint_code` for a raw diff line: tabs expand to stops every `tab` columns, and the byte
/// ranges of `spans` and `marks` (into the raw text) move with them.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_line(
    p: &Painter,
    left_center: Pos2,
    text: &str,
    spans: &[Span],
    marks: Option<(&[Range<u32>], Color32)>,
    font: &FontId,
    fg: Color32,
    tab: usize,
) {
    let e = tabs::expand(text, tab);
    if matches!(e.text, std::borrow::Cow::Borrowed(_)) {
        return paint_code(p, left_center, text, spans, marks, font, fg);
    }
    let spans = remap(spans, &e);
    let ranges = marks.map(|(m, c)| (m.iter().map(|r| e.map(r.start)..e.map(r.end)).collect::<Vec<_>>(), c));
    let marks = ranges.as_ref().map(|(m, c)| (m.as_slice(), *c));
    paint_code(p, left_center, &e.text, &spans, marks, font, fg);
}

/// `spans` moved onto the expanded text; a span out of range stays out of range.
fn remap(spans: &[Span], e: &Expanded) -> Vec<Span> {
    spans.iter().map(|s| Span { start: e.map(s.start), end: e.map(s.end), kind: s.kind }).collect()
}

/// Draws `text` left-aligned and vertically centred on `left_center`. Runs of `spans` get their
/// token color, everything else `fg`. With no colored spans (or ranges that don't fit the
/// text) this is the plain single-color call, so plain diffs draw exactly as before.
///
/// `marks` are changed-word byte ranges, filled with their color under the text.
pub(super) fn paint_code(
    p: &Painter,
    left_center: Pos2,
    text: &str,
    spans: &[Span],
    marks: Option<(&[Range<u32>], Color32)>,
    font: &FontId,
    fg: Color32,
) {
    let plain = || {
        p.text(left_center, Align2::LEFT_CENTER, text, font.clone(), fg);
    };
    let colored = !spans.iter().all(|s| s.kind == TokenKind::Plain);
    let job = colored.then(|| layout_job(text, spans, font, fg)).flatten();
    match marks.filter(|(m, _)| !m.is_empty()) {
        None => {
            let Some(job) = job else { return plain() };
            let galley = p.layout_job(job);
            let top = left_center.y - galley.size().y / 2.0;
            p.galley(pos2(left_center.x, top), galley, fg);
        }
        Some((marks, mark_color)) => {
            let job = job.unwrap_or_else(|| LayoutJob::simple_singleline(text.to_owned(), font.clone(), fg));
            let galley = p.layout_job(job);
            let origin = pos2(left_center.x, left_center.y - galley.size().y / 2.0);
            paint_marks(p, origin, &galley, text, marks, mark_color);
            p.galley(origin, galley, fg);
        }
    }
}

/// Fills each mark's extent in `galley` (laid out at `origin`); invalid ranges are skipped.
fn paint_marks(p: &Painter, origin: Pos2, galley: &Galley, text: &str, marks: &[Range<u32>], color: Color32) {
    let height = galley.size().y;
    for (c0, c1) in marks.iter().filter_map(|r| char_span(text, r)) {
        let x0 = galley.pos_from_ccursor(CCursor::new(c0)).min.x;
        let x1 = galley.pos_from_ccursor(CCursor::new(c1)).min.x;
        let rect = Rect::from_min_max(pos2(origin.x + x0, origin.y), pos2(origin.x + x1, origin.y + height));
        p.rect_filled(rect, 2.0, color);
    }
}

/// The char indices `(start, end)` of a byte range, or `None` if it is reversed, out of bounds
/// or not on char boundaries.
fn char_span(text: &str, r: &Range<u32>) -> Option<(usize, usize)> {
    let (start, end) = (r.start as usize, r.end as usize);
    if start > end {
        return None;
    }
    let c0 = text.get(..start)?.chars().count();
    Some((c0, c0 + text.get(start..end)?.chars().count()))
}

/// One section per span, plus any gaps and the tail in `fg`; `None` if a span is out of
/// bounds, out of order or not on a char boundary.
fn layout_job(text: &str, spans: &[Span], font: &FontId, fg: Color32) -> Option<LayoutJob> {
    let mut job = LayoutJob::default();
    let mut add = |s: &str, color: Color32| {
        if !s.is_empty() {
            job.append(s, 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
        }
    };
    let mut pos = 0;
    for span in spans {
        let (start, end) = (span.start as usize, span.end as usize);
        if start < pos {
            return None;
        }
        add(text.get(pos..start)?, fg);
        add(text.get(start..end)?, kind_color(span.kind, fg));
        pos = end;
    }
    add(text.get(pos..)?, fg);
    Some(job)
}

fn kind_color(k: TokenKind, fg: Color32) -> Color32 {
    match k {
        TokenKind::Plain => fg,
        k => color::token(k),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: u32, end: u32, kind: TokenKind) -> Span {
        Span { start, end, kind }
    }

    #[test]
    fn job_covers_the_text_and_rejects_bad_ranges() {
        let font = FontId::monospace(12.0);
        let spans = [span(0, 3, TokenKind::Keyword), span(4, 5, TokenKind::Number)];
        let job = layout_job("let 1;", &spans, &font, Color32::WHITE).unwrap();
        assert_eq!(job.text, "let 1;");
        assert_eq!(job.sections.len(), 4, "keyword, gap, number, tail");
        assert!(layout_job("ab", &[span(0, 3, TokenKind::String)], &font, Color32::WHITE).is_none(), "past the end");
        assert!(layout_job("é", &[span(0, 1, TokenKind::String)], &font, Color32::WHITE).is_none(), "inside a char");
        let unordered = [span(2, 3, TokenKind::String), span(0, 1, TokenKind::String)];
        assert!(layout_job("abc", &unordered, &font, Color32::WHITE).is_none());
    }

    #[test]
    fn spans_after_a_tab_move_with_it() {
        // "\tlet x" with tab 4: `let` at bytes 1..4 is drawn at 4..7.
        let e = tabs::expand("\tlet x", 4);
        assert_eq!(e.text, "    let x");
        let spans = [span(0, 1, TokenKind::Plain), span(1, 4, TokenKind::Keyword), span(4, 6, TokenKind::Plain)];
        let moved = remap(&spans, &e);
        assert_eq!(moved, [span(0, 4, TokenKind::Plain), span(4, 7, TokenKind::Keyword), span(7, 9, TokenKind::Plain)]);
        assert_eq!(&e.text[4..7], "let");
        assert_eq!(remap(&[span(2, 40, TokenKind::String)], &e)[0].end, u32::MAX, "past the end stays bad");
        assert!(layout_job(&e.text, &moved, &FontId::monospace(12.0), Color32::WHITE).is_some());
    }

    #[test]
    fn char_span_maps_bytes_and_skips_bad_ranges() {
        assert_eq!(char_span("é-x", &(3..4)), Some((2, 3)));
        assert_eq!(char_span("abc", &(0..3)), Some((0, 3)));
        assert_eq!(char_span("abc", &(2..5)), None, "past the end");
        assert_eq!(char_span("abc", &(4..5)), None, "start past the end");
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = 2..1;
        assert_eq!(char_span("abc", &reversed), None, "reversed");
        assert_eq!(char_span("é", &(1..2)), None, "inside a char");
        assert_eq!(char_span("é", &(0..1)), None, "ends inside a char");
    }
}
