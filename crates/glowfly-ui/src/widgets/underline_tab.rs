//! A row of underline tabs (the pull request view's Overview, Commits and Files changed): each
//! tab a quiet target (the hover layer, the inset focus ring) whose label is `weak` at rest and
//! `text` when hovered or selected; the selected tab has a 2 px accent line under its label, at
//! the tab's bottom, on the 1 px `border` line under the whole row. Selection is shown by the
//! line, never by a filled block (`docs/COLOUR.md`, "The accent").

use super::paint_text;
use super::quiet_target::{self, State};
use crate::theme::color;
use crate::theme::sizes::ControlSize;
use crate::theme::type_scale;
use eframe::egui::{Align2, Rect, Response, Sense, Shape, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};

/// A tab's height and its padding either side of the label.
const SIZE: ControlSize = ControlSize::Regular;
const PAD_X: f32 = 12.0;
/// The selected tab's line.
const LINE_H: f32 = 2.0;

/// Draws `items` as a row of tabs across the width of `ui`, `current` selected, and returns the
/// tab clicked, if it isn't `current`. `after` draws in the same row, after the tabs.
pub fn row<T: Copy + PartialEq>(ui: &mut Ui, current: T, items: &[(T, String)], after: impl FnOnce(&mut Ui)) -> Option<T> {
    // The baseline goes under the tabs, so the selected tab's line covers it.
    let under = ui.painter().add(Shape::Noop);
    let left = ui.max_rect().left();
    let right = ui.max_rect().right();
    let r = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let mut picked = None;
        for (value, label) in items {
            if tab(ui, *value == current, label).clicked() && *value != current {
                picked = Some(*value);
            }
        }
        after(ui);
        picked
    });
    let y = r.response.rect.bottom() - 0.5;
    ui.painter().set(under, Shape::line_segment([pos2(left, y), pos2(right, y)], Stroke::new(1.0_f32, color::border())));
    r.inner
}

/// Draws one tab labelled `label`, `selected` or not, and returns its response.
fn tab(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let font = type_scale::font(SIZE.metrics().text);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color::text());
    let size = vec2(galley.size().x + 2.0 * PAD_X, SIZE.metrics().h);
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    r.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let s = State::of(&r, false);
        quiet_target::paint(p, rect, SIZE.metrics().radius, SIZE, s);
        let ink = if selected || s.lit() { color::text() } else { color::weak() };
        let text = Rect::from_center_size(rect.center(), galley.size());
        paint_text(p, text.left_center(), Align2::LEFT_CENTER, label, type_scale::font(SIZE.metrics().text), ink);
        if selected {
            let line = Rect::from_min_max(pos2(text.left(), rect.bottom() - LINE_H), pos2(text.right(), rect.bottom()));
            p.rect_filled(line, 0.0, color::accent());
        }
    }
    r
}
