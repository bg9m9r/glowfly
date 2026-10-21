//! A dialog's side pane for a less familiar operation (`dialog_frame`'s guide): a tinted panel
//! beside the form that explains the operation in a few numbered steps under "How it works".
//! It sits right of the form from under the header to the footer's bottom, or, when the window
//! has no room for both, stacked full width above the form. Its text is plain labels, so a
//! screen reader reads it after the form.

use crate::ui::theme::tokens::radius;
use crate::ui::theme::{color, type_scale};
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, vec2};
use glowfly_core::i18n::Lang;

/// The pane's width beside the form, and its gap from the form.
pub(super) const PANE_W: f32 = 272.0;
pub(super) const GAP: f32 = 24.0;
/// The pane's inner padding.
const INNER: f32 = 16.0;
/// A step number's circle.
const BADGE: f32 = 20.0;
/// Between the badge and its text.
const BADGE_GAP: f32 = 10.0;
/// Between the heading and the first step, and between steps.
const STEP_GAP: f32 = 12.0;
const RADIUS: u8 = radius::LG;

/// The steps a guided dialog explains (`dialog_guides.rs`), already in the dialog's language.
pub struct Guide {
    pub steps: Vec<&'static str>,
}

/// Where the pane goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Placement {
    /// Right of the form.
    Beside,
    /// Above the form, full width (a narrow window).
    Stacked,
}

/// Beside the form when the window's `room` holds the form's base width, the gap and the pane.
pub(super) fn place(form_base: f32, room: f32) -> Placement {
    if room >= form_base + GAP + PANE_W { Placement::Beside } else { Placement::Stacked }
}

fn heading_font(ui: &Ui) -> FontId {
    type_scale::strong(ui, type_scale::BODY)
}

fn step_font() -> FontId {
    type_scale::font(type_scale::BODY)
}

/// The width a step's text wraps at in a pane `width` wide.
fn text_w(width: f32) -> f32 {
    (width - 2.0 * INNER - BADGE - BADGE_GAP).max(40.0)
}

/// The pane's height at `width`: the same galleys `paint` lays out, so exact on the first frame.
pub(super) fn height(ui: &Ui, t: Lang, guide: &Guide, width: f32) -> f32 {
    let layout = |text: &str, font: FontId, wrap: f32| ui.fonts(|f| f.layout(text.to_owned(), font, Color32::PLACEHOLDER, wrap).size().y);
    let inner = (width - 2.0 * INNER).max(40.0);
    let mut h = 2.0 * INNER + layout(t.tr("dialog.guide.heading"), heading_font(ui), inner);
    for step in &guide.steps {
        h += STEP_GAP + layout(step, step_font(), text_w(width)).max(BADGE);
    }
    h
}

/// Paints the pane into `rect`.
pub(super) fn paint(ui: &mut Ui, t: Lang, guide: &Guide, rect: Rect) {
    ui.painter().rect_filled(rect, CornerRadius::same(RADIUS), color::guide_fill());
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect.shrink(INNER)).layout(Layout::top_down(Align::Min)));
    let ui = &mut child;
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add(egui::Label::new(RichText::new(t.tr("dialog.guide.heading")).font(heading_font(ui)).color(color::text())).wrap());
    let wrap = text_w(rect.width());
    for (i, step) in guide.steps.iter().enumerate() {
        ui.add_space(STEP_GAP);
        let galley = ui.fonts(|f| f.layout((*step).to_owned(), step_font(), color::text(), wrap));
        // A row is as tall as its text, or the badge when that is taller.
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), galley.size().y.max(BADGE)), Sense::hover());
        let badge = Rect::from_min_size(row.min, vec2(BADGE, BADGE));
        badge_paint(ui, badge, i + 1);
        let text_rect = Rect::from_min_size(row.min + vec2(BADGE + BADGE_GAP, 0.0), vec2(wrap, row.height()));
        let mut text = ui.new_child(UiBuilder::new().max_rect(text_rect).layout(Layout::top_down(Align::Min)));
        text.add(egui::Label::new(RichText::new(*step).font(step_font()).color(color::text())).wrap());
    }
}

/// A step's number in an accent ring.
fn badge_paint(ui: &Ui, rect: Rect, n: usize) {
    let p = ui.painter();
    p.circle_stroke(rect.center(), BADGE / 2.0 - 0.75, Stroke::new(1.5_f32, color::accent()));
    let font = type_scale::strong(ui, type_scale::SMALL);
    p.text(rect.center(), Align2::CENTER_CENTER, n.to_string(), font, color::text());
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn beside_needs_room_for_form_gap_and_pane() {
        assert_eq!(place(480.0, 776.0), Placement::Beside);
        assert_eq!(place(480.0, 775.0), Placement::Stacked);
        assert_eq!(place(640.0, 935.0), Placement::Stacked);
    }

    const LAST: &str = "Step four wraps over more than one line, because a pane only 272 px wide has room for a few words a line.";

    #[test]
    fn height_matches_the_painted_steps() {
        let mut h = Harness::builder().with_size(vec2(400.0, 600.0)).build_ui_state(
            |ui, rect: &mut Rect| {
                let g = Guide { steps: vec!["One short step.", "A second step, a little longer than the first.", "Three.", LAST] };
                let height = height(ui, Lang::En, &g, PANE_W);
                *rect = Rect::from_min_size(pos2(8.0, 8.0), vec2(PANE_W, height));
                paint(ui, Lang::En, &g, *rect);
            },
            Rect::NOTHING,
        );
        h.run();
        let rect = *h.state();
        let last = h.get_by_label(LAST).bounding_box().expect("the last step has bounds");
        assert!(last.y1 as f32 <= rect.bottom() - INNER + 0.5, "the last step ends at {} in a pane ending at {}", last.y1, rect.bottom());
        assert!(last.y1 as f32 >= rect.bottom() - INNER - 0.5, "no space left over: {} in {}", last.y1, rect.bottom());
    }
}
