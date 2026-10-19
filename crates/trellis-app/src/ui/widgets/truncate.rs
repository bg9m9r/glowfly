//! Single-line text cut to a width with an ellipsis, with the whole text on hover when cut.
//!
//! egui ends a cut line on the last whole glyph that fits followed by "…" and marks the galley
//! `elided`; `galley.text()` keeps the whole text, for tooltips and accessible names.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Color32, FontId, Galley, Painter, Pos2, Rect, Response, Sense, TextStyle, TextWrapMode, Ui, WidgetText, pos2, vec2,
};
use std::sync::Arc;

/// The overflow character egui draws at the end of a cut line.
const ELLIPSIS: char = '…';

/// `text` as a one-line job in `font` and `color`, ready to be cut.
pub fn cut_job(text: impl Into<String>, font: FontId, color: Color32) -> LayoutJob {
    LayoutJob::simple_singleline(text.into(), font, color)
}

/// `job` laid out on one line no wider than `max_w`, ending in "…" when it doesn't fit.
pub fn cut_galley(ctx: &egui::Context, mut job: LayoutJob, max_w: f32) -> Arc<Galley> {
    job.wrap = TextWrapping::truncate_at_width(max_w.max(0.0));
    ctx.fonts(|f| f.layout_job(job))
}

/// Where a cut line was painted, and whether it was cut.
#[derive(Clone, Copy, Debug)]
pub struct Cut {
    pub rect: Rect,
    pub cut: bool,
}

impl Default for Cut {
    /// Nothing painted (a hidden cell).
    fn default() -> Self {
        Self { rect: Rect::NOTHING, cut: false }
    }
}

impl Cut {
    /// Whether the pointer is over a cut line (one that has the whole text to show on hover).
    pub fn hovered(&self, pointer: Option<Pos2>) -> bool {
        self.cut && pointer.is_some_and(|p| self.rect.contains(p))
    }
}

/// Paints `job` from `at` (its left end, vertically centred on `at.y`), cut to `max_w`. Below
/// the width of one "…" (or for a NaN width) nothing is painted, and non-empty text counts as cut.
pub fn paint_cut(p: &Painter, at: Pos2, job: LayoutJob, max_w: f32) -> Cut {
    let Some(first) = job.sections.first() else {
        return Cut { rect: Rect::from_min_size(at, vec2(0.0, 0.0)), cut: false };
    };
    let fallback = first.format.color;
    let ellipsis_w = p.ctx().fonts(|f| f.glyph_width(&first.format.font_id, ELLIPSIS));
    if max_w.is_nan() || max_w < ellipsis_w {
        return Cut { rect: Rect::from_min_size(at, vec2(0.0, 0.0)), cut: !job.text.is_empty() };
    }
    let galley = cut_galley(p.ctx(), job, max_w);
    let min = pos2(at.x, at.y - galley.size().y / 2.0);
    let cut = Cut { rect: Rect::from_min_size(min, galley.size()), cut: galley.elided };
    p.galley(min, galley, fallback);
    cut
}

/// One line of `text` in `font` and `color`, painted as `paint_cut` does.
pub fn paint_cut_text(p: &Painter, at: Pos2, text: &str, font: FontId, color: Color32, max_w: f32) -> Cut {
    paint_cut(p, at, cut_job(text, font, color), max_w)
}

/// A non-selectable one-line label cut to `max_w` (and the room left), named by its whole text.
/// Returns its response and whether it was cut; it attaches no tooltip, so the caller decides.
pub fn cut_label(ui: &mut Ui, text: impl Into<WidgetText>, max_w: f32) -> (Response, bool) {
    let width = max_w.min(ui.available_width()).max(0.0);
    let galley = text.into().into_galley(ui, Some(TextWrapMode::Truncate), width, TextStyle::Body);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    let full = galley.text().to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), &full));
    let cut = galley.elided;
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, galley, ui.visuals().text_color());
    }
    (resp, cut)
}

/// `cut_label` with the whole text on hover when it was cut.
pub fn truncate_label(ui: &mut Ui, text: impl Into<WidgetText>, max_w: f32) -> Response {
    let text = text.into();
    let full = text.text().to_owned();
    let (resp, cut) = cut_label(ui, text, max_w);
    if cut { resp.on_hover_text(full) } else { resp }
}

/// A text field's hint cut to the field: `TextEdit::hint_text` keeps a laid-out galley as it
/// is (it lays a plain hint out on one uncut line). Drawn in egui's weak hint colour. Returns
/// the hint and whether it was cut, for the caller's hover with the whole hint.
pub fn hint(ui: &Ui, text: &str, field_w: f32) -> (WidgetText, bool) {
    // The field keeps a 4 px margin on each side of its text.
    let room = field_w.min(ui.available_width() - 8.0);
    let job = cut_job(text, TextStyle::Body.resolve(ui.style()), Color32::PLACEHOLDER);
    let galley = cut_galley(ui.ctx(), job, room);
    let cut = galley.elided;
    (WidgetText::Galley(galley), cut)
}

/// A row's tooltip: its own `tip`, with the whole `text` as the first line when the row's text
/// was cut (the whole text alone when it has no tooltip of its own).
pub fn full_tip(cut: bool, text: &str, tip: Option<String>) -> Option<String> {
    match (cut, tip) {
        (true, Some(tip)) => Some(format!("{text}\n{tip}")),
        (true, None) => Some(text.to_owned()),
        (false, tip) => tip,
    }
}

#[cfg(test)]
#[path = "../snapshot_tests_truncate.rs"]
mod snapshot_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::text::TextFormat;
    use eframe::egui::{Align, LayerId};

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        ctx
    }

    fn font() -> FontId {
        FontId::proportional(13.0)
    }

    /// The characters a galley draws, the "…" included.
    fn drawn(g: &Galley) -> String {
        g.rows.iter().flat_map(|r| r.glyphs.iter().map(|gl| gl.chr)).collect()
    }

    const LONG: &str = "Move the settings loader into its own module so the preferences window, the command line and the \
                        tests share one place to read it";

    #[test]
    fn short_text_is_not_cut() {
        let ctx = ctx();
        let g = cut_galley(&ctx, cut_job("Short", font(), Color32::WHITE), 200.0);
        assert!(!g.elided);
        assert_eq!(drawn(&g), "Short");
    }

    #[test]
    fn long_text_ends_on_whole_glyphs_and_an_ellipsis() {
        let ctx = ctx();
        assert!(LONG.chars().count() >= 110, "{}", LONG.len());
        let g = cut_galley(&ctx, cut_job(LONG, font(), Color32::WHITE), 80.0);
        assert!(g.elided);
        assert!(g.size().x <= 80.0, "{}", g.size().x);
        let shown = drawn(&g);
        let kept = shown.strip_suffix(ELLIPSIS).expect("ends with an ellipsis");
        assert!(!kept.is_empty() && LONG.starts_with(kept), "{shown:?}");
        assert_eq!(g.text(), LONG, "the galley keeps the whole text");
    }

    #[test]
    fn text_exactly_as_wide_as_the_room_is_not_cut() {
        let ctx = ctx();
        let natural = ctx.fonts(|f| f.layout_no_wrap("Fix build on Windows".into(), font(), Color32::WHITE)).size().x;
        let g = cut_galley(&ctx, cut_job("Fix build on Windows", font(), Color32::WHITE), natural);
        assert!(!g.elided);
        assert_eq!(drawn(&g), "Fix build on Windows");
    }

    #[test]
    fn a_cut_inside_the_second_section_keeps_the_first_whole() {
        let ctx = ctx();
        let mut job = LayoutJob::default();
        let summary = "Add app skeleton";
        job.append(summary, 0.0, TextFormat { valign: Align::Center, ..TextFormat::simple(font(), Color32::WHITE) });
        job.append(LONG, 10.0, TextFormat { valign: Align::Center, ..TextFormat::simple(FontId::proportional(12.5), Color32::GRAY) });
        let summary_w = ctx.fonts(|f| f.layout_no_wrap(summary.into(), font(), Color32::WHITE)).size().x;
        let g = cut_galley(&ctx, job, summary_w + 120.0);
        assert!(g.elided);
        assert!(drawn(&g).starts_with(summary), "{:?}", drawn(&g));
    }

    #[test]
    fn too_narrow_or_nan_paints_nothing_and_counts_as_cut() {
        let ctx = ctx();
        let p = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
        let at = pos2(10.0, 20.0);
        for w in [3.0, f32::NAN, -5.0] {
            let c = paint_cut_text(&p, at, "Search results page", font(), Color32::WHITE, w);
            assert!(c.cut, "{w}");
            assert_eq!(c.rect.width(), 0.0, "{w}");
        }
        assert!(!paint_cut_text(&p, at, "", font(), Color32::WHITE, 3.0).cut, "empty text is never cut");
        let wide = paint_cut_text(&p, at, "Short", font(), Color32::WHITE, 200.0);
        assert!(!wide.cut && wide.rect.width() > 0.0);
    }

    #[test]
    fn only_a_cut_line_is_hovered() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(50.0, 16.0));
        let inside = Some(pos2(10.0, 8.0));
        assert!(!Cut { rect, cut: false }.hovered(inside));
        assert!(Cut { rect, cut: true }.hovered(inside));
        assert!(!Cut { rect, cut: true }.hovered(Some(pos2(80.0, 8.0))));
        assert!(!Cut { rect, cut: true }.hovered(None));
    }

    #[test]
    fn full_tip_puts_the_whole_text_first_when_cut() {
        assert_eq!(full_tip(true, "name", Some("path".into())).as_deref(), Some("name\npath"));
        assert_eq!(full_tip(true, "name", None).as_deref(), Some("name"));
        assert_eq!(full_tip(false, "name", Some("path".into())).as_deref(), Some("path"));
        assert_eq!(full_tip(false, "name", None), None);
    }
}
