//! The toolbar's controls: a tool button (icon over caption), a toggle that stays latched while
//! its panel or mode is on, a split button (a tool button and its dropdown caret, one shape
//! with two targets, `segment_bar.rs`), the borderless group a row of them sits in, and the
//! divider between groups. Every slot is a quiet target that lights its whole shape through
//! `button_look::paint_target`: the hover layer, the pressed layer while held (a darker fill,
//! never a move), the latched layer while on or open, and the inset focus ring on its corners.
//! The caret glyph here is the one every toolbar dropdown shares, the pickers' too. A disabled
//! slot paints its enabled colours at half alpha, and a split button may carry a count badge
//! (Badge 18) at its icon's top right, cut out of the icon by a ring in the ground under it.

use super::super::theme::color;
use super::button_look::{Press, Target, layer_on, paint_target};
use super::chip::{Chip, ChipSize, Look, rect_at};
use super::segment_bar::{BarStyle, Seg, segment_bar};
use super::{Icon, paint_icon, paint_text};
use crate::theme::color::Palette;
use crate::theme::sizes::ControlSize;
use crate::theme::tokens::{BORDER_W, CARET_HIT_W};
use crate::theme::type_scale;
use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};

/// A tool slot's width with captions shown; a longer caption widens it to the caption plus
/// `CAPTION_PAD`.
const TOOL_W: f32 = 56.0;
/// A tool slot's width with captions hidden.
const TOOL_W_BARE: f32 = 40.0;
/// A split button's slot with captions hidden when it can carry a badge (Pull, Push), badge
/// or not, so the bar never moves when one appears.
const TOOL_W_BADGED_BARE: f32 = 52.0;
/// A tool slot's height, the toolbar row's whole height.
pub(crate) const TOOL_H: f32 = 48.0;
/// Room beside a caption inside its slot, both sides together: 5 px a side, so a hover tile
/// never hugs a long caption and two captions' ink stays 12 px apart.
const CAPTION_PAD: f32 = 10.0;
/// The icon's centre and the caption's, below the slot's top.
const ICON_Y: f32 = 17.0;
const CAPTION_Y: f32 = 37.0;
/// The size whose corners and focus style a tool slot takes.
const TOOL_SIZE: ControlSize = ControlSize::Regular;
/// Between the buttons of one toolbar group (egui's item spacing there); a dropdown caret sits
/// right against its button.
pub const TOOL_SPACING: f32 = 2.0;
// Two captions side by side keep at least 8 px between them.
const _: () = assert!(CAPTION_PAD + TOOL_SPACING >= 8.0);
/// Between two toolbar groups: the divider with its margins (`ghost::divider`).
pub const GROUP_GAP: f32 = super::ghost::DIVIDER_W;
/// The toolbar row's height: a tool slot with room above and below it.
const GROUP_PAD: f32 = 3.0;
pub const GROUP_H: f32 = TOOL_H + 2.0 * (GROUP_PAD + BORDER_W);
/// A dropdown caret button's width.
pub const CARET_W: f32 = CARET_HIT_W;
/// The caret glyph: a stroked chevron `CARET_ARM` either side of its centre and `CARET_DROP`
/// tall, in the tab glyphs' line width.
pub const CARET_ARM: f32 = 3.5;
const CARET_DROP: f32 = 2.0;
/// The line between a split button's halves: an icon's height, shorter than the divider between
/// groups, so a caret reads as part of its action.
const DIVIDER_H: f32 = TOOL_SIZE.metrics().icon;
/// After a split button's caret, before the next slot of its group: more than `TOOL_SPACING`,
/// so the caret sits closer to its own action than to the next one.
pub const SPLIT_GAP: f32 = 8.0;
/// A disabled slot's alpha: its enabled colours at half strength.
const DISABLED_ALPHA: f32 = 0.5;
/// A badge's top below the slot's top, its left edge right of the icon's centre, its least room
/// to the slot's right edge (`badged_min`), and the knockout ring around it.
const BADGE_TOP: f32 = 2.0;
const BADGE_X: f32 = 4.0;
const BADGE_EDGE: f32 = 2.0;
const BADGE_RING: f32 = 2.0;
/// The largest count a badge shows as it is; above it, "9+" (the exact count is in the branch
/// picker, the tooltip and the accessible name).
const BADGE_MAX: usize = 9;

/// A split button's count badge: `count` (hidden at 0) over a slot whose resting ground is
/// `ground` (the toolbar's fill), which the badge's knockout ring is painted in.
#[derive(Clone, Copy, Debug)]
pub struct Badge {
    pub count: usize,
    pub ground: Color32,
}

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

/// A disabled slot's colour for `c`, its enabled colour.
fn disabled(c: Color32) -> Color32 {
    c.gamma_multiply(DISABLED_ALPHA)
}

/// Paints a tool slot's icon and, with `labels`, its caption under it: the icon in text colour,
/// the caption in full text colour while `lit` (hovered or on) and `weak` otherwise; disabled,
/// the icon's and the caption's enabled colours at half alpha.
fn paint_tool(p: &Painter, rect: Rect, (icon, label): (Icon, &str), enabled: bool, lit: bool, labels: bool) {
    let col = if enabled { color::text() } else { disabled(color::text()) };
    if labels {
        let caption = if !enabled {
            disabled(color::weak())
        } else if lit {
            color::text()
        } else {
            color::weak()
        };
        paint_icon(p, pos2(rect.center().x, rect.top() + ICON_Y), icon, col);
        paint_text(p, pos2(rect.center().x, rect.top() + CAPTION_Y), Align2::CENTER_CENTER, label, type_scale::font(type_scale::SMALL), caption);
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

/// The width `split_button` gives a button captioned `label` that can carry a badge: wider
/// with captions hidden (`TOOL_W_BADGED_BARE`), and never narrower than a "9+" badge needs
/// (`badged_min`), so the badge clears the icon.
pub fn badged_tool_width(ui: &Ui, label: &str, labels: bool) -> f32 {
    let w = if labels { tool_width(ui, label, labels) } else { TOOL_W_BADGED_BARE };
    w.max(badged_min(Chip::new(ChipSize::Badge, badge_text(BADGE_MAX + 1)).width(ui.ctx())))
}

/// The least slot that keeps a badge `widest` wide (the "9+" one, whatever the count shown, so
/// the bar never moves) at its place right of the centred icon, `BADGE_EDGE` inside the slot.
fn badged_min(widest: f32) -> f32 {
    2.0 * (BADGE_X + widest + BADGE_EDGE)
}

/// A tool button with its dropdown caret right against it: one shape with two targets, parted
/// by a line that always shows. Only the hovered half lights, a disabled button never does, the
/// half being held takes the pressed layer and the caret stays latched while its menu is
/// `open`. The caret takes the disabled look when neither the button nor anything in its menu
/// can be used (`menu_live`). A `badge` shows its count over the button's icon. Returns the
/// button's and the caret's responses.
pub fn split_button(
    ui: &mut Ui,
    tool: (Icon, &str, &str),
    enabled: bool,
    labels: bool,
    (menu_live, open): (bool, bool),
    badge: Option<Badge>,
) -> (Response, Response) {
    let (icon, label, name) = tool;
    let w = if badge.is_some() { badged_tool_width(ui, label, labels) } else { tool_width(ui, label, labels) };
    let segs = [Seg { w, enabled }, Seg { w: CARET_W, enabled: true }];
    let bar = segment_bar(ui, TOOL_SIZE, TOOL_H, TOOL_SIZE.metrics().radius, BarStyle::Quiet { divider: DIVIDER_H }, &segs);
    bar.paint(ui, &[false, open]);
    let mut responses = bar.responses.into_iter();
    let (button, menu) = (responses.next().expect("the button"), responses.next().expect("the caret"));
    paint_tool(ui.painter(), button.rect, (icon, label), enabled, enabled && button.hovered(), labels);
    if let Some(b) = badge {
        let press = if enabled { Press::of(button.hovered(), button.is_pointer_button_down_on(), false) } else { Press::Rest };
        paint_badge(ui.painter(), button.rect, b, press);
    }
    super::super::access::name_button(&button, enabled, name);
    let tone = if enabled || menu_live { color::weak() } else { disabled(color::weak()) };
    caret(ui.painter(), menu.rect.center(), tone);
    let button = if enabled { button.on_hover_cursor(egui::CursorIcon::PointingHand) } else { button };
    (button, menu.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// A badge's text for `count`: the count, or "9+" above 9.
fn badge_text(count: usize) -> String {
    if count > BADGE_MAX { format!("{BADGE_MAX}+") } else { count.to_string() }
}

/// The rect of a badge `w` wide over the icon of the tool slot `slot`: its top `BADGE_TOP`
/// below the slot's, its left always `BADGE_X` right of the icon's centre (never over the
/// arrowhead; `badged_tool_width` leaves the room). The same with captions shown or hidden.
fn badge_rect(slot: Rect, w: f32) -> Rect {
    let x = slot.center().x + BADGE_X;
    let h = ChipSize::Badge.metrics().h;
    rect_at(ChipSize::Badge, x, slot.top() + BADGE_TOP + h / 2.0, w)
}

/// Paints `badge` over the icon of the tool slot `slot` in state `press`: nothing at 0, else
/// the count as a Badge with a ring in the colour the slot shows under it. Lit (hovered,
/// pressed or latched), the badge's fill is the ground, since the lit layer matches `raised`;
/// in the light and high-contrast themes it has a 1 px edge in the input border colour (the
/// plain border is too faint on the hover layer), which is what sets it apart there.
fn paint_badge(p: &Painter, slot: Rect, badge: Badge, press: Press) {
    if badge.count == 0 {
        return;
    }
    let chip = Chip::new(ChipSize::Badge, badge_text(badge.count)).look(badge_look(&color::current(), badge.ground, press));
    let rect = badge_rect(slot, chip.width(p.ctx()));
    let ring = CornerRadius::same(ChipSize::Badge.metrics().radius.saturating_add(BADGE_RING as u8));
    p.rect_filled(rect.expand(BADGE_RING), ring, layer_on(badge.ground, press));
    chip.paint_in_rect(p, rect);
}

/// A badge's look in palette `pal` over `ground` in state `press`.
fn badge_look(pal: &Palette, ground: Color32, press: Press) -> Look {
    let fill = if press == Press::Rest { pal.raised } else { ground };
    let look = Look::new(fill, pal.text);
    if pal.high_contrast || !pal.dark { look.edged(pal.input_border) } else { look }
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

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
