//! Custom-painted widget controls: toolbar buttons and their spacing, chips, nav rows, icon
//! buttons; left-panel section headers (with their hover + and ⋯, and the empty state line) are
//! in `section_header.rs`, icon shapes in `icon.rs`, text cut to a width with an ellipsis
//! is in `truncate.rs`, the shared rich tooltip and egui's tooltip style in `tooltip.rs`, its
//! rise into place in `tooltip_motion.rs`, and the segmented control in `segmented.rs`. The
//! banner style is in `banner.rs`, field and form messages in `form_message.rs`, and the error,
//! warning and information marks in `severity.rs`. The shared row selection and hover look is
//! in `row_fill.rs`, the weak lines that say why an action is disabled in `reason.rs`, and the
//! menu items marked with an info icon whose reason shows on hover or focus (or that open a
//! setting first) in `menu_reason.rs`. The tab bars' painted close × and
//! new-tab + are in `tab_glyph.rs`, and the quiet, clickable chips of the commit details (its
//! SHA, parents and committer) in `quiet_chip.rs`. The quiet headers' ghost buttons, divider
//! and icon tooltips (the diff header, the terminal panel) are in `ghost.rs`. A dialog's 36 px
//! text field is `text_field.rs`, and its label-field-help group `field_group.rs`.

pub mod banner;
pub mod buttons;
pub mod field_group;
pub mod form_message;
pub mod ghost;
mod icon;
pub mod menu_reason;
pub mod quiet_chip;
pub mod reason;
pub mod row_fill;
pub mod section_header;
pub mod segmented;
pub mod severity;
pub mod tab_glyph;
pub mod text_field;
pub mod tooltip;
pub mod tooltip_motion;
pub mod truncate;

pub use super::list_row::{
    GROUP_ROW_H, GUTTER_X, LIST_ROW_H, NESTED_GUTTER_X, NESTED_INDENT, PAD_X, Part, ROW_INDENT, SECTION_H, TITLE_X, file_row,
    file_row_with_end, list_row, list_row_cut, list_row_hidden, row_text, row_text_reserving,
};
pub use field_group::FieldGroup;
pub use icon::{Icon, paint_icon, paint_icon_sized};
pub use quiet_chip::quiet_chip;
pub use section_header::section_fonts;
#[cfg(test)]
pub use section_header::section_header;
pub use text_field::TextField;
pub use truncate::{Cut, cut_label, paint_cut, paint_cut_text, truncate_label};

use super::theme::color;
use crate::ui::theme::type_scale;

use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

/// A tool slot's width with captions shown; a longer caption widens it to the caption plus
/// `CAPTION_PAD`.
pub const TOOL_W: f32 = 56.0;
/// A tool slot's width with captions hidden.
pub const TOOL_W_BARE: f32 = 40.0;
/// A tool slot's height, the toolbar row's whole height.
pub const TOOL_H: f32 = 48.0;
/// Room beside a caption inside its slot, both sides together.
pub const CAPTION_PAD: f32 = 8.0;
/// The icon's centre and the caption's, below the slot's top.
pub const ICON_Y: f32 = 17.0;
pub const CAPTION_Y: f32 = 37.0;
/// The corners of a slot's hover fill, pressed fill and focus ring.
pub const TOOL_RADIUS: u8 = 6;
/// Between the buttons of one toolbar group (egui's item spacing there); a dropdown caret sits
/// right against its button.
pub const TOOL_SPACING: f32 = 2.0;
/// Between two toolbar group surfaces: with their padding and stroke, 16 px from a button to
/// the next group's first one.
pub const GROUP_GAP: f32 = 8.0;
/// A toolbar group surface's padding inside its 1 px stroke, and its corners.
pub const GROUP_PAD: i8 = 3;
pub const GROUP_RADIUS: u8 = 10;
/// A toolbar group's height: a button, its padding and its stroke.
pub const GROUP_H: f32 = TOOL_H + 2.0 * (GROUP_PAD as f32 + 1.0);

/// Toolbar button: the icon with its caption under it (captions optional). `name` is its
/// accessible name, which includes the caption; the caller attaches the tooltip
/// (`tooltip::attach`). A disabled button still takes the keyboard focus, so Tab reaches its
/// reason.
pub fn tool_button(ui: &mut Ui, icon: Icon, label: &str, name: &str, enabled: bool, labels: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() | Sense::FOCUSABLE };
    let (rect, resp) = ui.allocate_exact_size(vec2(tool_width(ui, label, labels), TOOL_H), sense);
    let p = ui.painter();
    let slot = rect.shrink(1.0);
    let radius = CornerRadius::same(TOOL_RADIUS);
    if enabled && resp.is_pointer_button_down_on() {
        p.rect_filled(slot, radius, color::selected());
    } else if enabled && resp.hovered() {
        p.rect_filled(slot, radius, color::hover());
    }
    if resp.has_focus() {
        p.rect_stroke(slot, radius, Stroke::new(1.5_f32, color::accent()), StrokeKind::Inside);
    }
    let col = if enabled { color::text() } else { color::faint() };
    if labels {
        let caption = if !enabled {
            color::faint()
        } else if resp.hovered() {
            color::text()
        } else {
            color::weak()
        };
        paint_icon(p, pos2(rect.center().x, rect.top() + ICON_Y), icon, col);
        p.text(pos2(rect.center().x, rect.top() + CAPTION_Y), Align2::CENTER_CENTER, label, type_scale::font(type_scale::SMALL), caption);
    } else {
        paint_icon(p, rect.center(), icon, col);
    }
    super::access::name_button(&resp, enabled, name);
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

/// One toolbar group: its buttons in a row on a quiet surface (the window's background, a 1 px
/// border, rounded corners), `TOOL_SPACING` apart. The row keeps the parent's direction, so a
/// group works in a right-to-left layout too.
pub fn tool_group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(color::bg())
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

/// A dropdown caret button's width.
pub const CARET_W: f32 = 14.0;

/// Small dropdown caret button right against its tool button (no spacing between them), with
/// the caret at the button's icon height.
pub fn caret_button(ui: &mut Ui, labels: bool) -> Response {
    ui.add_space(-ui.spacing().item_spacing.x);
    let (rect, resp) = ui.allocate_exact_size(vec2(CARET_W, TOOL_H), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect.shrink(1.0), CornerRadius::same(TOOL_RADIUS), color::hover());
    }
    let c = if labels { pos2(rect.center().x, rect.top() + ICON_Y) } else { rect.center() };
    p.add(egui::Shape::convex_polygon(
        vec![pos2(c.x - 4.0, c.y - 2.0), pos2(c.x + 4.0, c.y - 2.0), pos2(c.x, c.y + 3.0)],
        color::weak(),
        Stroke::NONE,
    ));
    resp
}

pub fn chevron(p: &Painter, c: Pos2, open: bool, col: Color32) {
    let pts = if open {
        vec![pos2(c.x - 4.0, c.y - 2.0), pos2(c.x + 4.0, c.y - 2.0), pos2(c.x, c.y + 3.0)]
    } else {
        vec![pos2(c.x - 2.0, c.y - 4.0), pos2(c.x + 3.0, c.y), pos2(c.x - 2.0, c.y + 4.0)]
    };
    p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
}

/// Rounded label chip drawn with the painter; returns its width.
pub fn chip(p: &Painter, x: f32, y: f32, text: &str, fill: Color32, fg: Color32) -> f32 {
    let g = p.layout_no_wrap(text.to_owned(), type_scale::font(type_scale::CAPTION), fg);
    let w = g.size().x + 12.0;
    let r = Rect::from_min_size(pos2(x, y - 10.0), vec2(w, 20.0));
    p.rect_filled(r, CornerRadius::same(3), fill);
    let gh = g.size().y;
    p.galley(pos2(x + 6.0, y - gh / 2.0), g, fg);
    w
}

/// Dashed circle (used for the WIP node).
pub fn dashed_circle(p: &Painter, c: Pos2, r: f32, stroke: Stroke) {
    let n = 12;
    for i in 0..n {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = a0 + std::f32::consts::TAU / n as f32 * 0.55;
        p.line_segment([pos2(c.x + r * a0.cos(), c.y + r * a0.sin()), pos2(c.x + r * a1.cos(), c.y + r * a1.sin())], stroke);
    }
}

pub fn initials(name: &str) -> String {
    let mut parts = name.split_whitespace().filter_map(|w| w.chars().next());
    match (parts.next(), parts.next_back()) {
        (Some(a), Some(b)) => format!("{}{}", a.to_uppercase(), b.to_uppercase()),
        (Some(a), None) => a.to_uppercase().to_string(),
        _ => "?".into(),
    }
}

/// Preferences navigation row: icon + label in body text, highlighted when selected.
pub fn nav_row(ui: &mut Ui, icon: Option<Icon>, text: &str, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), type_scale::BODY + 20.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0.0, color::selected());
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    let mut x = rect.left() + 14.0;
    if let Some(icon) = icon {
        paint_icon(p, pos2(x + 9.0, rect.center().y), icon, if selected { color::strong() } else { color::weak() });
        x += 30.0;
    }
    p.text(
        pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        type_scale::font(type_scale::BODY),
        if selected { color::strong() } else { color::text() },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small square button showing a painted icon (for glyphs the font lacks).
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    // Disabled (e.g. inside `add_enabled_ui`): dimmed, inert, and still explained on hover.
    let enabled = ui.is_enabled();
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let fill = if enabled && resp.hovered() { color::hover() } else { color::raised() };
    p.rect_filled(rect, CornerRadius::same(3), fill);
    // The toolbar icons are drawn for ~20px; scale down by drawing at the center.
    paint_icon(p, rect.center(), icon, if enabled { color::text() } else { color::faint() });
    resp.on_hover_text(tooltip).on_disabled_hover_text(tooltip)
}

/// A small clickable painted icon with a tooltip, for icons the fonts lack (star, cross).
pub fn small_icon(ui: &mut Ui, icon: Icon, tooltip: &str, col: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tooltip));
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(3), color::hover());
    }
    paint_icon(ui.painter(), rect.center(), icon, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tooltip)
}
