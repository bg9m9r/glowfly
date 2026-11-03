//! Form controls drawn in the palette's colours: text fields with a visible border and a focus
//! ring (padded and sized by a `ControlSize` through `SizedField`), and checkboxes and radio buttons that are always a visible box or circle.
//!
//! They size, sense and describe themselves (accesskit role, label, toggled state) exactly as
//! egui's own widgets do, so swapping one in moves nothing. `tests/view_checks/controls.rs`
//! checks that views use these instead of egui's.

use super::color;
use super::sizes::ControlSize;
use super::tokens::{self, radius};
use eframe::egui::{
    self, Align, CornerRadius, Galley, Margin, NumExt, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextStyle, Ui, Vec2, WidgetInfo,
    WidgetText, WidgetType, pos2, vec2,
};
use glowfly_core::settings::FocusStyle;
use std::sync::Arc;

/// A text field with a 1 px `input_border` frame (weak on hover) and a 2 px accent focus ring,
/// with the Strong `tokens::focus_ring` outside it under the Strong focus style. Only the frame
/// strokes change: sizes, ids and focus behave as the plain `TextEdit`.
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
        if r.has_focus() && tokens::focus_style(ui.ctx()) == FocusStyle::Strong {
            // egui reports the text's rect; the frame is the whole widget's, and its 2 px focus
            // edge reaches 1 px outside it.
            let frame = ui.ctx().read_response(r.id).map_or(r.rect, |w| w.rect).expand(1.0);
            let corners = saved.visuals.widgets.inactive.corner_radius;
            tokens::focus_ring(ui.painter(), frame, CornerRadius::same(radius::around(corners.nw, 1)));
        }
        ui.set_style(saved);
        r
    }
}

/// Adds `edit` as a themed [`Field`] with egui's own padding. Inline and dialog fields take a
/// size instead: [`SizedField`].
pub fn field(ui: &mut Ui, edit: egui::TextEdit<'_>) -> Response {
    ui.add(Field(edit))
}

/// A field's padding at the text's left and right, in both field sizes.
pub const FIELD_PAD_X: f32 = 10.0;

/// The padding above and below a field's text. A single-line field is also centred in the
/// size's height, so its padding is only the least room (and grows the field with a larger
/// font); a multiline field's rows start below it.
pub(super) const fn field_pad_y(size: ControlSize, multiline: bool) -> f32 {
    match (size, multiline) {
        (ControlSize::Large, _) => 8.0,
        (_, true) => 6.0,
        (_, false) => 4.0,
    }
}

/// A field's padding in `size`, with `trailing` px right of the text instead of the side
/// padding (room for a button painted inside the field).
pub fn field_margin(size: ControlSize, multiline: bool, trailing: Option<f32>) -> Margin {
    let (x, y) = (FIELD_PAD_X as i8, field_pad_y(size, multiline) as i8);
    Margin { left: x, right: trailing.map_or(x, |t| t as i8), top: y, bottom: y }
}

/// The text's width in a field `outer` px wide (with the side padding).
pub fn field_text_width(outer: f32) -> f32 {
    (outer - 2.0 * FIELD_PAD_X).max(0.0)
}

/// A [`Field`] at a control size: Regular (28 px: the inline fields, such as filters, find, the
/// commit message and Preferences forms) or Large (36 px: dialog fields). Its padding comes
/// from the size, never from the view, and a single-line field centres its text. `width` is
/// the whole field's; the response's rect is the whole field, padding included (egui's own is
/// the text's).
pub struct SizedField<'t> {
    edit: egui::TextEdit<'t>,
    size: ControlSize,
    multiline: bool,
    trailing: Option<f32>,
    width: Option<f32>,
}

impl<'t> SizedField<'t> {
    /// `edit` at `size`, Regular or Large; it fills the row unless given a `width`.
    pub fn new(size: ControlSize, edit: egui::TextEdit<'t>) -> Self {
        debug_assert!(matches!(size, ControlSize::Regular | ControlSize::Large), "a field is Regular or Large, not {size:?}");
        Self { edit, size, multiline: false, trailing: None, width: None }
    }

    /// `edit` is multiline: its rows start at the top padding instead of being centred.
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// The whole field's width, padding included.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Keeps `w` px right of the text instead of the side padding, for a button painted inside
    /// the field.
    pub fn trailing_room(mut self, w: f32) -> Self {
        self.trailing = Some(w);
        self
    }
}

impl egui::Widget for SizedField<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let SizedField { edit, size, multiline, trailing, width } = self;
        let margin = field_margin(size, multiline, trailing);
        let align = if multiline { Align::Min } else { Align::Center };
        let inner = width.map_or(f32::INFINITY, |w| (w - margin.sum().x).max(0.0));
        let edit = edit.margin(margin).min_size(vec2(0.0, size.metrics().h)).vertical_align(align).desired_width(inner);
        let mut r = ui.add(Field(edit));
        r.rect += margin;
        r
    }
}

/// A checkbox: an input-coloured box with an `input_border` edge, filled with accent (or its
/// `tint`) and a check mark (or a dash when indeterminate) when on.
pub struct Check<'a> {
    checked: &'a mut bool,
    text: WidgetText,
    indeterminate: bool,
    tint: Option<egui::Color32>,
    /// The least hit area; the box stays `icon_width`.
    hit: Vec2,
    /// Where the box's centre goes, when it has no text, instead of the hit area's left.
    box_at: Option<egui::Pos2>,
    /// What screen readers call it, instead of its text.
    name: Option<String>,
}

impl<'a> Check<'a> {
    pub fn new(checked: &'a mut bool, text: impl Into<WidgetText>) -> Self {
        Self { checked, text: text.into(), indeterminate: false, tint: None, hit: Vec2::ZERO, box_at: None, name: None }
    }

    pub fn without_text(checked: &'a mut bool) -> Self {
        Self::new(checked, WidgetText::default())
    }

    /// Fills with `col` instead of the accent when on (the conflict view's sides).
    pub fn tint(mut self, col: egui::Color32) -> Self {
        self.tint = Some(col);
        self
    }

    /// A hit area at least `size` (the conflict view's 24 px), the box keeping its size.
    pub fn hit(mut self, size: Vec2) -> Self {
        self.hit = size;
        self
    }

    /// Paints the box (of a check without text) centred on `at`, wherever its hit area is.
    pub fn box_at(mut self, at: egui::Pos2) -> Self {
        self.box_at = Some(at);
        self
    }

    /// Names it for screen readers (the shown text, if any, stays).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Shows a dash instead of the check mark (partly on).
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }
}

impl egui::Widget for Check<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Check { checked, text, indeterminate, tint, hit, box_at, name } = self;
        let centre = box_at.filter(|_| text.is_empty());
        let (rect, galley, mut response) = layout(ui, text, hit);
        if response.clicked() {
            *checked = !*checked;
            response.mark_changed();
        }
        let label = name.as_deref().unwrap_or_else(|| galley.as_ref().map_or("", |g| g.text()));
        response.widget_info(|| {
            if indeterminate {
                WidgetInfo::labeled(WidgetType::Checkbox, ui.is_enabled(), label)
            } else {
                WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *checked, label)
            }
        });
        if ui.is_rect_visible(rect) {
            let at = centre.map_or(rect, |c| Rect::from_center_size(c, Vec2::splat(ui.spacing().icon_width)));
            let big = paint_box(ui, at, (*checked, indeterminate, tint), response.hovered());
            if response.has_focus() {
                tokens::focus_ring(ui.painter(), big, radius::SM);
            }
            paint_label(ui, rect, galley, &response);
        }
        response
    }
}

/// Paints a checkbox's box at the left of `rect`, vertically centred, with no widget of its own:
/// for a row whose one response is the checkbox and carries its widget info.
pub fn paint_check(ui: &Ui, rect: Rect, checked: bool, hovered: bool) {
    paint_box(ui, rect, (checked, false, None), hovered);
}

/// Paints the box (with a check mark when `checked`, a dash when `indeterminate`, filled with
/// `tint` or the accent); returns it.
fn paint_box(ui: &Ui, rect: Rect, (checked, indeterminate, tint): (bool, bool, Option<egui::Color32>), hovered: bool) -> Rect {
    let p = color::current();
    let (_, big) = ui.spacing().icon_rectangles(rect);
    let (fill, stroke) = match tint {
        Some(c) if checked || indeterminate => (c, Stroke::new(1.0_f32, c)),
        _ => icon_colors(&p, checked || indeterminate, hovered),
    };
    ui.painter().rect(big, CornerRadius::same(radius::SM), fill, stroke, StrokeKind::Inside);
    // A tinted fill can be light (the conflict view's theirs): its mark takes whichever of the
    // input and text colours stands out more on it.
    let mark = Stroke::new(2.0_f32, if tint.is_some() { mark_on(&p, fill) } else { p.input });
    let (l, r, t, b, w) = (big.left(), big.right(), big.top(), big.bottom(), big.width());
    if indeterminate {
        ui.painter().add(Shape::hline((l + 0.25 * w)..=(r - 0.25 * w), big.center().y, mark));
    } else if checked {
        let points = vec![pos2(l + 0.22 * w, big.center().y), pos2(l + 0.42 * w, b - 0.26 * w), pos2(r - 0.2 * w, t + 0.26 * w)];
        ui.painter().add(Shape::line(points, mark));
    }
    big
}

/// The check mark's colour on `fill`: the input colour or the text colour, whichever contrasts
/// more with it.
pub(crate) fn mark_on(p: &color::Palette, fill: egui::Color32) -> egui::Color32 {
    use super::contrast::ratio;
    if ratio(p.input, fill) >= ratio(p.text, fill) { p.input } else { p.text }
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
        let (rect, galley, response) = layout(ui, text, Vec2::ZERO);
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
/// Allocates a check or radio with `text`, at least `hit`.
fn layout(ui: &mut Ui, text: WidgetText, hit: Vec2) -> (Rect, Option<Arc<Galley>>, Response) {
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
    size = size.at_least(hit);
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
