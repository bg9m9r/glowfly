//! Form controls drawn in the palette's colours: text fields with a visible border and a focus
//! ring, and checkboxes and radio buttons that are always a visible box or circle.
//!
//! They size, sense and describe themselves (accesskit role, label, toggled state) exactly as
//! egui's own widgets do, so swapping one in moves nothing. `tests/controls.rs` checks that
//! views use these instead of egui's.

use super::color;
use super::tokens::{self, radius};
use eframe::egui::{
    self, CornerRadius, Galley, NumExt, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextStyle, Ui, Vec2, WidgetInfo, WidgetText,
    WidgetType, pos2, vec2,
};
use std::sync::Arc;

/// A text field with a 1 px `input_border` frame (weak on hover) and a 2 px accent focus ring.
/// Only the frame strokes change: sizes, ids and focus behave as the plain `TextEdit`.
pub struct Field<'t>(pub egui::TextEdit<'t>);

impl egui::Widget for Field<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let p = color::current();
        let saved = ui.style().clone();
        let v = ui.visuals_mut();
        v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.input_border);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.weak);
        v.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.weak);
        v.selection.stroke = Stroke::new(2.0_f32, p.accent);
        // No child ui or scope: auto ids and layout stay those of an unwrapped TextEdit.
        let r = ui.add(self.0);
        ui.set_style(saved);
        r
    }
}

/// Adds `edit` as a themed [`Field`].
pub fn field(ui: &mut Ui, edit: egui::TextEdit<'_>) -> Response {
    ui.add(Field(edit))
}

/// A checkbox: an input-coloured box with an `input_border` edge, filled with accent and a
/// check mark (or a dash when indeterminate) when on.
pub struct Check<'a> {
    checked: &'a mut bool,
    text: WidgetText,
    indeterminate: bool,
}

impl<'a> Check<'a> {
    pub fn new(checked: &'a mut bool, text: impl Into<WidgetText>) -> Self {
        Self { checked, text: text.into(), indeterminate: false }
    }

    pub fn without_text(checked: &'a mut bool) -> Self {
        Self::new(checked, WidgetText::default())
    }

    /// Shows a dash instead of the check mark (partly on).
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }
}

impl egui::Widget for Check<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Check { checked, text, indeterminate } = self;
        let (rect, galley, mut response) = layout(ui, text);
        if response.clicked() {
            *checked = !*checked;
            response.mark_changed();
        }
        let label = galley.as_ref().map_or("", |g| g.text());
        response.widget_info(|| {
            if indeterminate {
                WidgetInfo::labeled(WidgetType::Checkbox, ui.is_enabled(), label)
            } else {
                WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *checked, label)
            }
        });
        if ui.is_rect_visible(rect) {
            let big = paint_box(ui, rect, *checked, indeterminate, response.hovered());
            if response.has_focus() {
                tokens::focus_ring(ui.painter(), big, radius::SM);
            }
            paint_label(ui, rect, galley, &response);
        }
        response
    }
}

/// A checkbox's box, as `Check` draws it, in the icon square at the left of `rect` (egui's
/// `icon_rectangles`), for items that draw their own label (the View menu's check items).
pub fn paint_check_box(ui: &Ui, rect: Rect, on: bool, hovered: bool) {
    paint_box(ui, rect, on, false, hovered);
}

/// Paints the box (with a check mark when `checked`, a dash when `indeterminate`); returns it.
fn paint_box(ui: &Ui, rect: Rect, checked: bool, indeterminate: bool, hovered: bool) -> Rect {
    let p = color::current();
    let (_, big) = ui.spacing().icon_rectangles(rect);
    let (fill, stroke) = icon_colors(&p, checked || indeterminate, hovered);
    ui.painter().rect(big, CornerRadius::same(radius::SM), fill, stroke, StrokeKind::Inside);
    let mark = Stroke::new(2.0_f32, p.input);
    let (l, r, t, b, w) = (big.left(), big.right(), big.top(), big.bottom(), big.width());
    if indeterminate {
        ui.painter().add(Shape::hline((l + 0.25 * w)..=(r - 0.25 * w), big.center().y, mark));
    } else if checked {
        let points = vec![pos2(l + 0.22 * w, big.center().y), pos2(l + 0.42 * w, b - 0.26 * w), pos2(r - 0.2 * w, t + 0.26 * w)];
        ui.painter().add(Shape::line(points, mark));
    }
    big
}

/// A radio button: an input-coloured circle with an `input_border` edge, or an accent circle
/// with an input-coloured dot when selected.
pub struct Radio {
    selected: bool,
    text: WidgetText,
}

impl Radio {
    pub fn new(selected: bool, text: impl Into<WidgetText>) -> Self {
        Self { selected, text: text.into() }
    }
}

impl egui::Widget for Radio {
    fn ui(self, ui: &mut Ui) -> Response {
        let Radio { selected, text } = self;
        let (rect, galley, response) = layout(ui, text);
        let label = galley.as_ref().map_or("", |g| g.text());
        response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, ui.is_enabled(), selected, label));
        if ui.is_rect_visible(rect) {
            let p = color::current();
            let (_, big) = ui.spacing().icon_rectangles(rect);
            let (c, w) = (big.center(), big.width());
            let (fill, stroke) = icon_colors(&p, selected, response.hovered());
            ui.painter().circle(c, w / 2.0, fill, stroke);
            if selected {
                ui.painter().circle_filled(c, w / 5.0, p.input);
            }
            if response.has_focus() {
                ui.painter().circle_stroke(c, w / 2.0 + 2.0, Stroke::new(2.0_f32, p.accent));
            }
            paint_label(ui, rect, galley, &response);
        }
        response
    }
}

/// A themed checkbox, as `ui.checkbox`.
pub fn checkbox(ui: &mut Ui, checked: &mut bool, text: impl Into<WidgetText>) -> Response {
    ui.add(Check::new(checked, text))
}

/// A themed radio button, as `ui.radio`.
pub fn radio(ui: &mut Ui, selected: bool, text: impl Into<WidgetText>) -> Response {
    ui.add(Radio::new(selected, text))
}

/// A themed radio button for one value of `current`, as `ui.radio_value`: a click on a
/// different value assigns `alt` and marks the response changed.
pub fn radio_value<V: PartialEq>(ui: &mut Ui, current: &mut V, alt: V, text: impl Into<WidgetText>) -> Response {
    let mut response = radio(ui, *current == alt, text);
    if response.clicked() && *current != alt {
        *current = alt;
        response.mark_changed();
    }
    response
}

/// The fill and edge of a box or circle: accent when on, otherwise input with an
/// `input_border` edge that turns weak on hover.
fn icon_colors(p: &color::Palette, on: bool, hovered: bool) -> (egui::Color32, Stroke) {
    if on {
        (p.accent, Stroke::new(1.0_f32, p.accent))
    } else {
        (p.input, Stroke::new(1.0_f32, if hovered { p.weak } else { p.input_border }))
    }
}

/// Lays out and allocates a checkbox or radio button exactly as egui's own do: the icon, the
/// label in the button text style, and at least the interact size.
fn layout(ui: &mut Ui, text: WidgetText) -> (Rect, Option<Arc<Galley>>, Response) {
    let spacing = ui.spacing();
    let (icon_width, icon_spacing, interact) = (spacing.icon_width, spacing.icon_spacing, spacing.interact_size);
    let (galley, mut size) = if text.is_empty() {
        (None, vec2(icon_width, 0.0))
    } else {
        let extra = vec2(icon_width + icon_spacing, 0.0);
        let galley = text.into_galley(ui, None, ui.available_width() - extra.x, TextStyle::Button);
        let size = (extra + galley.size()).at_least(interact);
        (Some(galley), size)
    };
    size = size.at_least(Vec2::splat(interact.y));
    size.y = size.y.max(icon_width);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    (rect, galley, response)
}

/// The label right of the icon, in the colour egui gives a widget in this interaction state.
fn paint_label(ui: &Ui, rect: Rect, galley: Option<Arc<Galley>>, response: &Response) {
    if let Some(galley) = galley {
        let s = ui.spacing();
        let pos = pos2(rect.min.x + s.icon_width + s.icon_spacing, rect.center().y - 0.5 * galley.size().y);
        ui.painter().galley(pos, galley, ui.style().interact(response).text_color());
    }
}
