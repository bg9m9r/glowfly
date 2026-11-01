//! The toolbar's controls: a tool button (icon over caption), a toggle that stays latched while
//! its panel or mode is on, a split button (a tool button and its dropdown caret, one shape
//! with two targets, `segment_bar.rs`), the borderless group a row of them sits in, and the
//! divider between groups. Every slot is a quiet target that lights its whole shape through
//! `button_look::paint_target`: the hover layer, the pressed layer while held (a darker fill,
//! never a move), the latched layer while on or open, and the inset focus ring on its corners.
//! The caret glyph here is the one every toolbar dropdown shares, the pickers' too.

use super::super::theme::color;
use super::button_look::{Press, Target, paint_target};
use super::segment_bar::{BarStyle, Seg, segment_bar};
use super::{Icon, paint_icon};
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens::{BORDER_W, CARET_HIT_W};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};

/// A tool slot's width with captions shown; a longer caption widens it to the caption plus
/// `CAPTION_PAD`.
const TOOL_W: f32 = 56.0;
/// A tool slot's width with captions hidden.
const TOOL_W_BARE: f32 = 40.0;
/// A tool slot's height, the toolbar row's whole height.
pub(crate) const TOOL_H: f32 = 48.0;
/// Room beside a caption inside its slot, both sides together.
const CAPTION_PAD: f32 = 8.0;
/// The icon's centre and the caption's, below the slot's top.
const ICON_Y: f32 = 17.0;
const CAPTION_Y: f32 = 37.0;
/// The size whose corners and focus style a tool slot takes.
const TOOL_SIZE: ControlSize = ControlSize::Regular;
/// Between the buttons of one toolbar group (egui's item spacing there); a dropdown caret sits
/// right against its button.
pub const TOOL_SPACING: f32 = 2.0;
/// Between two toolbar groups: the divider with its margins (`ghost::divider`).
pub const GROUP_GAP: f32 = super::ghost::DIVIDER_W;
/// The toolbar row's height: a tool slot with room above and below it.
const GROUP_PAD: f32 = 3.0;
pub const GROUP_H: f32 = TOOL_H + 2.0 * (GROUP_PAD + BORDER_W);
/// A dropdown caret button's width.
pub const CARET_W: f32 = CARET_HIT_W;
/// The caret glyph: a stroked chevron `CARET_ARM` either side of its centre and `CARET_DROP`
/// tall, in the tab glyphs' line width.
const CARET_ARM: f32 = 3.5;
const CARET_DROP: f32 = 2.0;
/// The line between a split button's halves: an icon's height, shorter than the divider between
/// groups, so a caret reads as part of its action.
const DIVIDER_H: f32 = TOOL_SIZE.metrics().icon;
/// After a split button's caret, before the next slot of its group: more than `TOOL_SPACING`,
/// so the caret sits closer to its own action than to the next one.
pub const SPLIT_GAP: f32 = 8.0;

/// Toolbar button: the icon with its caption under it (captions optional). `name` is its
/// accessible name, which includes the caption; the caller attaches the tooltip
/// (`tooltip::attach`). A disabled button still takes the keyboard focus, so Tab reaches its
/// reason.
pub fn tool_button(ui: &mut Ui, icon: Icon, label: &str, name: &str, enabled: bool, labels: bool) -> Response {
    tool_slot(ui, (icon, label, name), enabled, labels, false)
}

/// A toolbar button that shows a panel or mode: while `on` it stays latched, with its icon and
/// caption in full text colour.
pub fn tool_toggle(ui: &mut Ui, icon: Icon, label: &str, name: &str, on: bool, labels: bool) -> Response {
    tool_slot(ui, (icon, label, name), true, labels, on)
}

fn tool_slot(ui: &mut Ui, (icon, label, name): (Icon, &str, &str), enabled: bool, labels: bool, on: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
    let (rect, resp) = ui.allocate_exact_size(vec2(tool_width(ui, label, labels), TOOL_H), sense);
    let press = if enabled { Press::of(resp.hovered(), resp.is_pointer_button_down_on(), on) } else { Press::Rest };
    let corners = CornerRadius::same(TOOL_SIZE.metrics().radius);
    paint_target(ui.painter(), Target { rect, corners, edge: None }, TOOL_SIZE, press, resp.has_focus());
    paint_tool(ui.painter(), rect, (icon, label), enabled, on || resp.hovered(), labels);
    super::super::access::name_button(&resp, enabled, name);
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Paints a tool slot's icon and, with `labels`, its caption under it: `faint` when disabled,
/// the caption in full text colour while `lit` (hovered or on) and `weak` otherwise.
fn paint_tool(p: &Painter, rect: Rect, (icon, label): (Icon, &str), enabled: bool, lit: bool, labels: bool) {
    let col = if enabled { color::text() } else { color::faint() };
    if labels {
        let caption = if !enabled {
            color::faint()
        } else if lit {
            color::text()
        } else {
            color::weak()
        };
        paint_icon(p, pos2(rect.center().x, rect.top() + ICON_Y), icon, col);
        p.text(pos2(rect.center().x, rect.top() + CAPTION_Y), Align2::CENTER_CENTER, label, type_scale::font(type_scale::SMALL), caption);
    } else {
        paint_icon(p, rect.center(), icon, col);
    }
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

/// A tool button with its dropdown caret right against it: one shape with two targets, parted
/// by a line that always shows. Only the hovered half lights, a disabled button never does, the
/// half being held takes the pressed layer and the caret stays latched while its menu is
/// `open`. The caret is faint when neither the button nor anything in its menu can be used
/// (`menu_live`). Returns the button's and the caret's responses.
pub fn split_button(
    ui: &mut Ui,
    tool: (Icon, &str, &str),
    enabled: bool,
    labels: bool,
    menu_live: bool,
    open: bool,
) -> (Response, Response) {
    let (icon, label, name) = tool;
    let segs = [Seg { w: tool_width(ui, label, labels), enabled }, Seg { w: CARET_W, enabled: true }];
    let bar = segment_bar(ui, TOOL_SIZE, TOOL_H, TOOL_SIZE.metrics().radius, BarStyle::Quiet { divider: DIVIDER_H }, &segs);
    bar.paint(ui, &[false, open]);
    let mut responses = bar.responses.into_iter();
    let (button, menu) = (responses.next().expect("the button"), responses.next().expect("the caret"));
    paint_tool(ui.painter(), button.rect, (icon, label), enabled, enabled && button.hovered(), labels);
    super::super::access::name_button(&button, enabled, name);
    let tone = if enabled || menu_live { color::weak() } else { color::faint() };
    caret(ui.painter(), menu.rect.center(), tone);
    let button = if enabled { button.on_hover_cursor(egui::CursorIcon::PointingHand) } else { button };
    (button, menu.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// The dropdown caret: a stroked chevron pointing down, centred on `c`.
pub fn caret(p: &Painter, c: Pos2, col: Color32) {
    let (a, d) = (CARET_ARM, CARET_DROP / 2.0);
    let pts = vec![pos2(c.x - a, c.y - d), pos2(c.x, c.y + d), pos2(c.x + a, c.y - d)];
    p.add(Shape::line(pts, Stroke::new(super::tab_glyph::STROKE, col)));
}

/// One toolbar group: its buttons in a row, borderless, `TOOL_SPACING` apart, set apart from
/// the next group by `tool_gap`. The row keeps the parent's direction, so a group works in a
/// right-to-left layout too.
pub fn tool_group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    // One button high, so a group never grows to the row around it.
    let row = if ui.layout().prefer_right_to_left() { egui::Layout::right_to_left } else { egui::Layout::left_to_right };
    ui.allocate_ui_with_layout(vec2(ui.available_width(), TOOL_H), row(egui::Align::Center), |ui| {
        ui.set_min_height(TOOL_H);
        ui.spacing_mut().item_spacing.x = TOOL_SPACING;
        add(ui)
    })
}

/// The room after a split button's caret when another slot of its group follows: `SPLIT_GAP`
/// in all, whatever the group's spacing.
pub fn split_gap(ui: &mut Ui) {
    ui.add_space(SPLIT_GAP - ui.spacing().item_spacing.x);
}

/// The divider between two toolbar groups, `GROUP_GAP` wide whatever the item spacing around it.
pub fn tool_gap(ui: &mut Ui) {
    let spacing = ui.spacing().item_spacing.x;
    ui.add_space(-spacing);
    super::ghost::divider(ui);
    ui.add_space(-spacing);
}
