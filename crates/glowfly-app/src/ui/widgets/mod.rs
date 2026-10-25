//! Custom-painted widget controls: toolbar buttons and their spacing, chips, nav rows, icon
//! buttons; left-panel section headers (with their hover + and ⋯, and the empty state line) are
//! in `section_header.rs`, icon shapes in `icon.rs`, text cut to a width with an ellipsis
//! is in `truncate.rs`, the shared rich tooltip and egui's tooltip style in `tooltip.rs`, its
//! rise into place in `tooltip_motion.rs`, and the segmented control in `segmented.rs`. The
//! banner style is in `banner.rs`, field and form messages in `form_message.rs`, and the error,
//! warning and information marks in `severity.rs`. The shared row selection and hover look is
//! in `row_fill.rs`, the weak lines that say why an action is disabled in `reason.rs`, and the
//! menu items marked with an info icon whose reason shows on hover or focus (or that open a
//! setting first) in `menu_reason.rs`. The toolbar's buttons, toggles, split buttons, caret and
//! group surfaces are in `tool.rs`. The tab bars' painted close × and new-tab + are in
//! `tab_glyph.rs`, and the quiet, clickable chips of the commit details (its
//! SHA, parents and committer) in `quiet_chip.rs`. The quiet headers' ghost buttons, divider
//! and icon tooltips (the diff header, the terminal panel) are in `ghost.rs`. A dialog's 36 px
//! text field is `text_field.rs`, and its label-field-help group `field_group.rs`. The button
//! family (primary, tonal, secondary, quiet, danger, icon-only) is `buttons.rs`, each state's
//! colours `button_look.rs`, and the split button (an action and its menu caret as one pill)
//! `button_split.rs`. The always-visible thin scroll bar and the edge fades of dialog bodies,
//! Preferences and the File view are `scroll_cue.rs`. Right-click menus that open below the row
//! they belong to are `row_menu.rs`, and the collapsed Details disclosure under a plain-words
//! message `details.rs`.

pub mod banner;
mod button_look;
pub mod button_split;
pub mod buttons;
pub mod details;
pub mod field_group;
pub mod form_message;
pub mod ghost;
pub mod icon;
pub mod menu_reason;
pub mod quiet_chip;
pub mod reason;
pub mod row_fill;
pub mod row_menu;
pub mod scroll_cue;
pub mod section_header;
pub mod segmented;
pub mod severity;
pub mod tab_glyph;
pub mod text_field;
mod tool;
pub mod tooltip;
pub mod tooltip_motion;
pub mod truncate;

pub use super::list_row::{
    GROUP_ROW_H, GUTTER_X, LIST_ROW_H, NESTED_GUTTER_X, NESTED_INDENT, PAD_X, Part, ROW_INDENT, SECTION_H, TITLE_X, file_row,
    file_row_focus, file_row_with_end, keyed_file_row_with_end, list_row, list_row_cut, list_row_hidden, row_text, row_text_hover,
    row_text_reserving,
};
pub use field_group::FieldGroup;
pub use icon::{Icon, paint_icon, paint_icon_sized};
pub use quiet_chip::quiet_chip;
pub use section_header::section_fonts;
#[cfg(test)]
pub use section_header::section_header;
pub use text_field::TextField;
pub use tool::*;
pub use truncate::{Cut, cut_label, paint_cut, paint_cut_text, truncate_label};

use super::theme::color;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;

use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

pub fn chevron(p: &Painter, c: Pos2, open: bool, col: Color32) {
    let pts = if open {
        vec![pos2(c.x - 4.0, c.y - 2.0), pos2(c.x + 4.0, c.y - 2.0), pos2(c.x, c.y + 3.0)]
    } else {
        vec![pos2(c.x - 2.0, c.y - 4.0), pos2(c.x + 3.0, c.y), pos2(c.x - 2.0, c.y + 4.0)]
    };
    p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
}

/// A pill-shaped label chip drawn with the painter; returns its width.
pub fn chip(p: &Painter, x: f32, y: f32, text: &str, fill: Color32, fg: Color32) -> f32 {
    chip_with_edge(p, x, y, text, fill, None, fg)
}

/// `chip` with a 1 px `edge` inside its outline, when there is one.
pub fn chip_with_edge(p: &Painter, x: f32, y: f32, text: &str, fill: Color32, edge: Option<Color32>, fg: Color32) -> f32 {
    let g = p.layout_no_wrap(text.to_owned(), type_scale::font(type_scale::CAPTION), fg);
    let w = g.size().x + 12.0;
    let r = Rect::from_min_size(pos2(x, y - 10.0), vec2(w, 20.0));
    let radius = CornerRadius::same(radius::pill(r.height()));
    p.rect_filled(r, radius, fill);
    if let Some(edge) = edge {
        p.rect_stroke(r, radius, Stroke::new(1.0_f32, edge), egui::StrokeKind::Inside);
    }
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

/// A small icon-only button (`buttons::icon_only`, 24 x 24, no fill until hovered) named and
/// tipped `tooltip`.
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    // Disabled (e.g. inside `add_enabled_ui`): faint, inert, and still explained on hover.
    ui.add(buttons::icon_only(icon, tooltip).small().enabled(ui.is_enabled()).why(Some(tooltip)))
}

/// A small clickable painted icon with a tooltip, for icons the fonts lack (star, cross).
pub fn small_icon(ui: &mut Ui, icon: Icon, tooltip: &str, col: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tooltip));
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(radius::SM), color::hover_layer());
    }
    paint_icon(ui.painter(), rect.center(), icon, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tooltip)
}

/// A small weak cross that removes something (a key in Preferences > Keyboard): a 14 px icon in
/// a 20 px box, weak at rest, strong on the hover layer when hovered or focused, named and
/// tipped `name`.
pub fn remove_icon(ui: &mut Ui, name: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    super::access::name_button(&resp, true, name);
    let p = ui.painter();
    let lit = resp.hovered() || resp.has_focus();
    if lit {
        p.rect_filled(rect, CornerRadius::same(radius::SM), color::hover_layer());
    }
    let ink = if lit { color::strong() } else { color::weak() };
    paint_icon_sized(p, rect.center(), Icon::Close, ink, icon::SIZE_SM);
    if resp.has_focus() {
        crate::ui::theme::tokens::focus_ring_inset(p, rect, radius::SM);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(name)
}

#[cfg(test)]
#[path = "../snapshot_tests/snapshot_tests_design.rs"]
mod snapshot_tests_design;
