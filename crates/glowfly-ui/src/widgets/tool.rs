//! The toolbar's controls: a tool button (icon over caption), a toggle that stays filled while
//! its panel or mode is on, a split button (a tool button and its dropdown caret, lit as one
//! pill), the group surface they sit on and the gap between groups. The caret glyph here is
//! the one every toolbar dropdown shares, the pickers' too.

use super::super::theme::color;
use super::super::theme::tokens::{self, radius};
use super::{Icon, paint_icon};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};

/// A tool slot's width with captions shown; a longer caption widens it to the caption plus
/// `CAPTION_PAD`.
const TOOL_W: f32 = 56.0;
/// A tool slot's width with captions hidden.
const TOOL_W_BARE: f32 = 40.0;
/// A tool slot's height, the toolbar row's whole height.
pub const TOOL_H: f32 = 48.0;
/// Room beside a caption inside its slot, both sides together.
const CAPTION_PAD: f32 = 8.0;
/// The icon's centre and the caption's, below the slot's top.
const ICON_Y: f32 = 17.0;
const CAPTION_Y: f32 = 37.0;
/// The corners of a slot's hover fill, pressed fill and focus ring.
const TOOL_RADIUS: u8 = radius::MD;
/// Between the buttons of one toolbar group (egui's item spacing there); a dropdown caret sits
/// right against its button.
pub const TOOL_SPACING: f32 = 2.0;
/// Between two toolbar group surfaces: with their padding and stroke, 18 px from a button to
/// the next group's first one.
pub const GROUP_GAP: f32 = 10.0;
/// A toolbar group surface's padding inside its 1 px stroke, and its corners: nested around
/// the slots' (their corners, the padding and the stroke).
pub const GROUP_PAD: i8 = 3;
pub const GROUP_RADIUS: u8 = radius::around(TOOL_RADIUS, GROUP_PAD as u8 + 1);
/// A toolbar group's height: a button, its padding and its stroke.
pub const GROUP_H: f32 = TOOL_H + 2.0 * (GROUP_PAD as f32 + 1.0);
/// A dropdown caret button's width.
pub const CARET_W: f32 = 16.0;
/// The caret glyph: a stroked chevron `CARET_ARM` either side of its centre and `CARET_DROP`
/// tall, in the tab glyphs' line width.
const CARET_ARM: f32 = 3.5;
const CARET_DROP: f32 = 2.0;
/// The line between a split button's halves while it is lit.
const DIVIDER_H: f32 = 24.0;

/// How a tool slot paints its background.
#[derive(Clone, Copy, PartialEq)]
enum Fill {
    /// Hover and pressed fills of its own.
    Own,
    /// A toggle: the pressed fill stays while `on`.
    Toggle { on: bool },
    /// None: a split button paints both halves together.
    Split,
}

/// Toolbar button: the icon with its caption under it (captions optional). `name` is its
/// accessible name, which includes the caption; the caller attaches the tooltip
/// (`tooltip::attach`). A disabled button still takes the keyboard focus, so Tab reaches its
/// reason.
pub fn tool_button(ui: &mut Ui, icon: Icon, label: &str, name: &str, enabled: bool, labels: bool) -> Response {
    tool_slot(ui, icon, label, name, enabled, labels, Fill::Own)
}

/// A toolbar button that shows a panel or mode: while `on` it keeps the pressed fill, with its
/// icon and caption in full text colour.
pub fn tool_toggle(ui: &mut Ui, icon: Icon, label: &str, name: &str, on: bool, labels: bool) -> Response {
    tool_slot(ui, icon, label, name, true, labels, Fill::Toggle { on })
}

fn tool_slot(ui: &mut Ui, icon: Icon, label: &str, name: &str, enabled: bool, labels: bool, fill: Fill) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
    let (rect, resp) = ui.allocate_exact_size(vec2(tool_width(ui, label, labels), TOOL_H), sense);
    let p = ui.painter();
    let slot = rect.shrink(1.0);
    let radius = CornerRadius::same(TOOL_RADIUS);
    let on = fill == Fill::Toggle { on: true };
    if fill != Fill::Split {
        if on {
            p.rect_filled(slot, radius, color::tool_pressed());
        } else if enabled && resp.is_pointer_button_down_on() {
            p.rect_filled(slot, radius, color::tool_press());
        } else if enabled && resp.hovered() {
            p.rect_filled(slot, radius, color::tool_hover());
        }
    }
    if resp.has_focus() {
        tokens::focus_ring_inset(p, slot, TOOL_RADIUS);
    }
    let col = if enabled { color::text() } else { color::faint() };
    if labels {
        let caption = if !enabled {
            color::faint()
        } else if on || resp.hovered() {
            color::text()
        } else {
            color::weak()
        };
        paint_icon(p, pos2(rect.center().x, rect.top() + ICON_Y), icon, col);
        p.text(pos2(rect.center().x, rect.top() + CAPTION_Y), Align2::CENTER_CENTER, label, type_scale::font(type_scale::SMALL), caption);
    } else {
        paint_icon(p, rect.center(), icon, col);
    }
    super::super::access::name_button(&resp, enabled, name);
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// The width `tool_button` gives a button captioned `label`.
pub fn tool_width(ui: &Ui, label: &str, labels: bool) -> f32 {
    if labels {
        let text_w = ui.painter().layout_no_wrap(label.to_owned(), type_scale::font(type_scale::SMALL), Color32::PLACEHOLDER).size().x;
        TOOL_W.max(text_w + CAPTION_PAD)
    } else {
        TOOL_W_BARE
    }
}

/// A tool button with its dropdown caret right against it. Hovering either half lights both as
/// one pill, parted by a short line; the half being pressed (or the caret while its menu is
/// `open`) takes the pressed fill. The caret is faint when neither the button nor anything in
/// its menu can be used (`menu_live`). Returns the button's and the caret's responses.
pub fn split_button(
    ui: &mut Ui,
    tool: (Icon, &str, &str),
    enabled: bool,
    labels: bool,
    menu_live: bool,
    open: bool,
) -> (Response, Response) {
    let (icon, label, name) = tool;
    let bg = ui.painter().add(Shape::Noop);
    let button = tool_slot(ui, icon, label, name, enabled, labels, Fill::Split);
    ui.add_space(-ui.spacing().item_spacing.x);
    let (rect, menu) = ui.allocate_exact_size(vec2(CARET_W, TOOL_H), Sense::click());
    let (left, right) = (button.rect.shrink(1.0), rect.shrink(1.0));
    let lit = (enabled && button.hovered()) || menu.hovered() || open;
    let mut shapes = Vec::new();
    if lit {
        let both = Rect::from_min_max(left.min, right.max);
        shapes.push(Shape::rect_filled(both, CornerRadius::same(TOOL_RADIUS), color::tool_hover()));
    }
    if enabled && button.is_pointer_button_down_on() {
        shapes.push(Shape::rect_filled(left, radius::left(TOOL_RADIUS), color::tool_press()));
    }
    if open {
        shapes.push(Shape::rect_filled(right, radius::right(TOOL_RADIUS), color::tool_pressed()));
    } else if menu.is_pointer_button_down_on() {
        shapes.push(Shape::rect_filled(right, radius::right(TOOL_RADIUS), color::tool_press()));
    }
    if lit {
        let x = rect.left();
        let y = rect.center().y;
        shapes
            .push(Shape::line_segment([pos2(x, y - DIVIDER_H / 2.0), pos2(x, y + DIVIDER_H / 2.0)], Stroke::new(1.0_f32, color::border())));
    }
    ui.painter().set(bg, Shape::Vec(shapes));
    let tone = if enabled || menu_live { color::weak() } else { color::faint() };
    caret(ui.painter(), rect.center(), tone);
    (button, menu.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// The dropdown caret: a stroked chevron pointing down, centred on `c`.
pub fn caret(p: &Painter, c: Pos2, col: Color32) {
    let (a, d) = (CARET_ARM, CARET_DROP / 2.0);
    let pts = vec![pos2(c.x - a, c.y - d), pos2(c.x, c.y + d), pos2(c.x + a, c.y - d)];
    p.add(Shape::line(pts, Stroke::new(super::tab_glyph::STROKE, col)));
}

/// One toolbar group: its buttons in a row on a quiet surface (the neutral tool backing, a 1 px
/// border, rounded corners), `TOOL_SPACING` apart. The row keeps the parent's direction, so a
/// group works in a right-to-left layout too.
pub fn tool_group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(color::tool_backing())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(GROUP_RADIUS)
        .inner_margin(GROUP_PAD)
        .show(ui, |ui| {
            // One button high, so a group never grows to the row around it.
            let row = if ui.layout().prefer_right_to_left() { egui::Layout::right_to_left } else { egui::Layout::left_to_right };
            ui.allocate_ui_with_layout(vec2(ui.available_width(), TOOL_H), row(egui::Align::Center), |ui| {
                ui.set_min_height(TOOL_H);
                ui.spacing_mut().item_spacing.x = TOOL_SPACING;
                add(ui)
            })
            .inner
        })
}

/// The space between two toolbar groups (whatever the item spacing between them).
pub fn tool_gap(ui: &mut Ui) {
    ui.add_space(GROUP_GAP - ui.spacing().item_spacing.x);
}
