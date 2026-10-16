//! Painting diff line text with syntax colors. The spans come precomputed with the diff
//! (`trellis_core::highlight`); this only lays them out in the palette's token colors, after
//! expanding tabs to the tab stops. Hunk headers get the same treatment for their function
//! context (`paint_header`). A wrapped line paints one visual line under another, each with
//! its slice of the spans and changed-word marks.

use super::wrap::Wrap;
use crate::ui::theme::color;
use eframe::egui::text::{CCursor, LayoutJob, TextFormat};
use eframe::egui::{Align2, Color32, FontId, Galley, Painter, Pos2, Rect, pos2};
use std::ops::Range;
use trellis_core::highlight::{Span, TokenKind};
use trellis_core::tabs::{self, Expanded};
use trellis_core::wrap::segments;

/// `paint_code` for a raw diff line: tabs expand to stops every `tab` columns, and the byte
/// ranges of `spans` and `marks` (into the raw text) move with them. With `wrap`, a line too
/// long for its columns continues on visual lines below, `left_center` being the first one's.
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
    wrap: Option<Wrap>,
) {
    let e = tabs::expand(text, tab);
    if matches!(e.text, std::borrow::Cow::Borrowed(_)) {
        return paint_wrapped(p, left_center, text, spans, marks, font, fg, wrap);
    }
    let spans = remap(spans, &e);
    let ranges = marks.map(|(m, c)| (m.iter().map(|r| e.map(r.start)..e.map(r.end)).collect::<Vec<_>>(), c));
    let marks = ranges.as_ref().map(|(m, c)| (m.as_slice(), *c));
    paint_wrapped(p, left_center, &e.text, &spans, marks, font, fg, wrap);
}

/// `paint_code`, or with `wrap` and a line that needs more than one visual line, `paint_code`
/// per visual line, `wrap.line_h` apart, each with its slice of `spans` and `marks`. Spans that
/// don't fit the text are dropped (the line paints plain, as unwrapped), and so are bad marks.
#[allow(clippy::too_many_arguments)]
fn paint_wrapped(
    p: &Painter,
    left_center: Pos2,
    text: &str,
    spans: &[Span],
    marks: Option<(&[Range<u32>], Color32)>,
    font: &FontId,
    fg: Color32,
    wrap: Option<Wrap>,
) {
    let segs: Vec<Range<usize>> = match wrap {
        Some(w) => segments(text, w.cols).collect(),
        None => Vec::new(),
    };
    let (Some(w), true) = (wrap, segs.len() > 1) else {
        return paint_code(p, left_center, text, spans, marks, font, fg);
    };
    let spans = if spans_fit(text, spans) { spans } else { &[] };
    let marks = marks.map(|(m, c)| (m.iter().filter(|r| r.start <= r.end && r.end as usize <= text.len()).cloned().collect::<Vec<_>>(), c));
    for (k, seg) in segs.iter().enumerate() {
        let seg_spans: Vec<Span> =
            spans.iter().filter_map(|s| clip(s.start..s.end, seg).map(|r| Span { start: r.start, end: r.end, kind: s.kind })).collect();
        let seg_marks = marks.as_ref().map(|(m, c)| (m.iter().filter_map(|r| clip(r.clone(), seg)).collect::<Vec<_>>(), *c));
        let at = pos2(left_center.x, left_center.y + k as f32 * w.line_h);
        paint_code(p, at, &text[seg.clone()], &seg_spans, seg_marks.as_ref().map(|(m, c)| (m.as_slice(), *c)), font, fg);
    }
}

/// The part of byte range `r` inside `seg`, relative to the segment's start; `None` when they
/// don't overlap.
fn clip(r: Range<u32>, seg: &Range<usize>) -> Option<Range<u32>> {
    let (s0, s1) = (seg.start as u32, seg.end as u32);
    let (start, end) = (r.start.max(s0), r.end.min(s1));
    (start < end).then(|| start - s0..end - s0)
}

/// Whether `spans` fit `text` as `layout_job` needs them: in order, in bounds and on char
/// boundaries.
fn spans_fit(text: &str, spans: &[Span]) -> bool {
    let mut pos = 0;
    for span in spans {
        let (start, end) = (span.start as usize, span.end as usize);
        if start < pos || text.get(pos..start).is_none() || text.get(start..end).is_none() {
            return false;
        }
        pos = end;
    }
    true
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
    append_spans(&mut job, text, spans, font, fg)?;
    Some(job)
}

/// Appends `text` to `job` as `layout_job` lays it out; `None` (with `job` partly filled) on a
/// bad span.
fn append_spans(job: &mut LayoutJob, text: &str, spans: &[Span], font: &FontId, fg: Color32) -> Option<()> {
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
    Some(())
}

/// Paints a hunk header left-aligned and vertically centred on `left_center`, tabs expanded.
/// With `ctx` (the function context's byte offset in `header` and its spans, relative to it),
/// the range part keeps the header color and the context takes the token colors, like code.
/// Without it, or when the spans don't fit, the whole header is drawn plain, as before.
pub(super) fn paint_header(p: &Painter, left_center: Pos2, header: &str, ctx: Option<(usize, &[Span])>, font: &FontId, tab: usize) {
    let e = tabs::expand(header, tab);
    let Some(job) = ctx.and_then(|(at, spans)| header_job(&e, at, spans, font, color::hunk_fg(), color::text())) else {
        p.text(left_center, Align2::LEFT_CENTER, e.text, font.clone(), color::hunk_fg());
        return;
    };
    let galley = p.layout_job(job);
    let top = left_center.y - galley.size().y / 2.0;
    p.galley(pos2(left_center.x, top), galley, color::hunk_fg());
}

/// The expanded header `e` with its range (up to the context at raw byte `at`) in `range_fg`
/// and the context's `spans` (relative to `at`) in token colors, gaps in `code_fg`. `None` when
/// `at` or a span doesn't fit the text.
fn header_job(e: &Expanded, at: usize, spans: &[Span], font: &FontId, range_fg: Color32, code_fg: Color32) -> Option<LayoutJob> {
    let at = u32::try_from(at).ok()?;
    let cut = e.map(at);
    let text = e.text.get(..cut as usize)?;
    let shift = |b: u32| at.checked_add(b).map(|b| e.map(b)).filter(|&m| m != u32::MAX).and_then(|m| m.checked_sub(cut));
    let remapped =
        spans.iter().map(|s| Some(Span { start: shift(s.start)?, end: shift(s.end)?, kind: s.kind })).collect::<Option<Vec<_>>>()?;
    let mut job = LayoutJob::default();
    if !text.is_empty() {
        job.append(text, 0.0, TextFormat { font_id: font.clone(), color: range_fg, ..Default::default() });
    }
    append_spans(&mut job, e.text.get(cut as usize..)?, &remapped, font, code_fg)?;
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
    fn clip_moves_ranges_into_the_segment() {
        let spans = [span(0, 3, TokenKind::Keyword), span(3, 10, TokenKind::String), span(12, 14, TokenKind::Number)];
        let seg = 2..6;
        let clipped: Vec<Span> =
            spans.iter().filter_map(|s| clip(s.start..s.end, &seg).map(|r| Span { start: r.start, end: r.end, kind: s.kind })).collect();
        assert_eq!(clipped, [span(0, 1, TokenKind::Keyword), span(1, 4, TokenKind::String)], "the number is outside");
        assert_eq!(clip(6..8, &seg), None, "touching the end is outside");
        assert_eq!(clip(0..2, &seg), None, "touching the start is outside");
    }

    #[test]
    fn spans_fit_rejects_bad_ranges_like_layout_job() {
        assert!(spans_fit("let 1;", &[span(0, 3, TokenKind::Keyword), span(4, 5, TokenKind::Number)]));
        assert!(spans_fit("", &[]));
        let bad: [(&str, &[Span]); 3] = [
            ("ab", &[span(0, 3, TokenKind::String)]),
            ("é", &[span(0, 1, TokenKind::String)]),
            ("abc", &[span(2, 3, TokenKind::String), span(0, 1, TokenKind::String)]),
        ];
        for (text, spans) in bad {
            assert!(!spans_fit(text, spans), "{text:?} {spans:?}");
            assert!(layout_job(text, spans, &FontId::monospace(12.0), Color32::WHITE).is_none());
        }
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

    /// Each section's text and color.
    fn sections(job: &LayoutJob) -> Vec<(&str, Color32)> {
        job.sections.iter().map(|s| (&job.text[s.byte_range.clone()], s.format.color)).collect()
    }

    #[test]
    fn header_job_colors_the_context_like_code() {
        let font = FontId::monospace(12.0);
        let (range, code) = (Color32::RED, Color32::WHITE);
        let header = "@@ -1 +1 @@ fn x";
        let spans = [span(0, 2, TokenKind::Keyword), span(2, 3, TokenKind::Plain), span(3, 4, TokenKind::Function)];
        let job = header_job(&tabs::expand(header, 4), 12, &spans, &font, range, code).unwrap();
        assert_eq!(job.text, header);
        let expected =
            [("@@ -1 +1 @@ ", range), ("fn", color::token(TokenKind::Keyword)), (" ", code), ("x", color::token(TokenKind::Function))];
        assert_eq!(sections(&job), expected);

        // A tab in the context expands to the stop, and the spans move with it.
        let header = "@@ -1 +1 @@ 	int x;";
        let spans = [span(0, 1, TokenKind::Plain), span(1, 4, TokenKind::Type), span(4, 7, TokenKind::Plain)];
        let job = header_job(&tabs::expand(header, 4), 12, &spans, &font, range, code).unwrap();
        assert_eq!(job.text, "@@ -1 +1 @@     int x;");
        assert!(sections(&job).contains(&("int", color::token(TokenKind::Type))), "{:?}", sections(&job));
        assert_eq!(sections(&job)[0], ("@@ -1 +1 @@ ", range));
    }

    #[test]
    fn header_job_rejects_spans_that_dont_fit() {
        let font = FontId::monospace(12.0);
        let job =
            |header: &str, at: usize, spans: &[Span]| header_job(&tabs::expand(header, 4), at, spans, &font, Color32::RED, Color32::WHITE);
        assert!(job("@@ -1 +1 @@ fn", 12, &[span(0, 99, TokenKind::Keyword)]).is_none(), "past the end");
        assert!(job("@@ -1 +1 @@ 	fn", 12, &[span(0, 99, TokenKind::Keyword)]).is_none(), "past the end, tabbed");
        assert!(job("@@ -1 +1 @@ é", 12, &[span(0, 1, TokenKind::String)]).is_none(), "inside a char");
        assert!(job("@@ -1 +1 @@ fn", 99, &[span(0, 1, TokenKind::Keyword)]).is_none(), "context past the end");
        assert!(job("@ é @ x", 3, &[span(0, 1, TokenKind::Keyword)]).is_none(), "context inside a char");
        assert!(job("@@ -1 +1 @@ fn", 12, &[span(0, 2, TokenKind::Keyword)]).is_some());
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
