//! Painting diff line text with syntax colors. The spans come precomputed with the diff
//! (`trellis_core::highlight`); this only lays them out in the palette's token colors.

use crate::ui::theme::color;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, pos2};
use trellis_core::highlight::{Span, TokenKind};

/// Draws `text` left-aligned and vertically centred on `left_center`. Runs of `spans` get their
/// token color, everything else `fg`. With no colored spans (or ranges that don't fit the
/// text) this is the plain single-color call, so plain diffs draw exactly as before.
pub(super) fn paint_code(p: &Painter, left_center: Pos2, text: &str, spans: &[Span], font: &FontId, fg: Color32) {
    let plain = || {
        p.text(left_center, Align2::LEFT_CENTER, text, font.clone(), fg);
    };
    if spans.iter().all(|s| s.kind == TokenKind::Plain) {
        return plain();
    }
    let Some(job) = layout_job(text, spans, font, fg) else { return plain() };
    let galley = p.layout_job(job);
    let top = left_center.y - galley.size().y / 2.0;
    p.galley(pos2(left_center.x, top), galley, fg);
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
}
