//! A Markdown code block (`ui/markdown.rs`): the code in the monospace font on a raised box,
//! never wrapped (long lines scroll sideways), in token colors when its language is known
//! (spans from `MarkdownCache`) and selectable like other text.

use super::theme::{color, type_scale};
use crate::ui::theme::tokens::radius;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, FontId, Stroke, Ui};
use glowfly_core::highlight::Span;

/// Draws `text` (tabs expanded) with `spans` per line when given.
pub(in crate::ui) fn block(ui: &mut Ui, text: &str, spans: Option<&[Vec<Span>]>) {
    let font = type_scale::mono(type_scale::BODY);
    let job = layout(text, spans, &font);
    egui::Frame::new()
        .fill(color::raised())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(radius::SM)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::horizontal().id_salt("md-code").auto_shrink([false, true]).show(ui, |ui| {
                ui.add(egui::Label::new(job).extend());
            });
        });
}

/// The text as one job: each line in its token colors, or all plain when `spans` is missing
/// or doesn't fit the text.
fn layout(text: &str, spans: Option<&[Vec<Span>]>, font: &FontId) -> LayoutJob {
    let shown = if text.is_empty() { " " } else { text };
    spans
        .and_then(|s| colored(shown, s, font))
        .unwrap_or_else(|| LayoutJob::simple_singleline(shown.to_owned(), font.clone(), color::text()))
}

fn colored(text: &str, spans: &[Vec<Span>], font: &FontId) -> Option<LayoutJob> {
    let mut job = LayoutJob::default();
    let mut add = |text: &str, color| {
        if !text.is_empty() {
            job.append(text, 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
        }
    };
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            add("\n", color::text());
        }
        let mut at = 0usize;
        for s in spans.get(i).map(Vec::as_slice).unwrap_or_default() {
            let (start, end) = (s.start as usize, s.end as usize);
            if start < at || end < start {
                return None;
            }
            add(line.get(at..start)?, color::text());
            add(line.get(start..end)?, color::token(s.kind));
            at = end;
        }
        add(line.get(at..)?, color::text());
    }
    Some(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::highlight::TokenKind;

    fn span(start: u32, end: u32, kind: TokenKind) -> Span {
        Span { start, end, kind }
    }

    #[test]
    fn spans_color_their_bytes_and_bad_spans_fall_back_to_plain() {
        let font = FontId::monospace(14.0);
        let spans = vec![vec![span(0, 2, TokenKind::Keyword)], vec![]];
        let job = colored("fn x\nok", &spans, &font).expect("fits");
        assert_eq!(job.text, "fn x\nok");
        assert_eq!(job.sections[0].format.color, color::token(TokenKind::Keyword));
        assert!(colored("ab", &[vec![span(0, 5, TokenKind::String)]], &font).is_none(), "past the end");
        assert!(colored("é", &[vec![span(0, 1, TokenKind::String)]], &font).is_none(), "inside a char");
        assert_eq!(layout("é", Some(&[vec![span(0, 1, TokenKind::String)]]), &font).text, "é");
    }
}
