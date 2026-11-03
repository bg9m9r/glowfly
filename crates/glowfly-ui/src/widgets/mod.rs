//! Custom-painted widget controls: toolbar buttons and their spacing, chips, nav rows, icon
//! buttons; left-panel section headers (with their hover + and ⋯, and the empty state line) are
//! in `section_header.rs`, icon shapes in `icon.rs`, text cut to a width with an ellipsis
//! is in `truncate.rs`, the shared rich tooltip and egui's tooltip style in `tooltip.rs`, its
//! rise into place in `tooltip_motion.rs`, and the segmented control in `segmented.rs`. The
//! banner style is in `banner.rs`, field and form messages in `form_message.rs`, and the error,
//! warning and information marks in `severity.rs`. The shared row selection and hover look is
//! in `row_fill.rs`, the weak lines that say why an action is disabled in `reason.rs`, and the
//! menu items marked with an info icon whose reason shows on hover or focus (or that open a
//! setting first) in `menu_reason.rs`, and a submenu's open delay, close grace and placement in
//! `menu_submenu.rs` (opened from the keyboard: `menu_submenu_keyed.rs`), and opening menus from
//! a button with the keyboard too in `menu_open.rs`. The toolbar's buttons, toggles, split buttons, caret,
//! borderless groups and the dividers between them are in `tool.rs`, and the flush segments of a
//! segment bar (the pickers, a split) in `segment_bar.rs`. The tab bars' painted close × and new-tab + are in
//! `tab_glyph.rs`, glyphs moved onto the pixel grid in `icon_snap.rs`, a quiet target a view
//! lays out itself (a tab, the title bar's menu button) in `quiet_target.rs`, and every chip and
//! badge (the two chip sizes, and the quiet, clickable chips of the commit details) in `chip.rs`. The quiet headers' ghost buttons, divider
//! and icon tooltips (the diff header, the terminal panel) are in `ghost.rs`. A dialog's 36 px
//! text field is `text_field.rs`, and its label-field-help group `field_group.rs`. The button
//! family (primary, tonal, secondary, quiet, danger, icon-only) is `buttons.rs`, each state's
//! colours `button_look.rs`, and the split button (an action and its menu caret as one pill)
//! `button_split.rs`. The always-visible thin scroll bar and the edge fades of dialog bodies,
//! Preferences and the File view are `scroll_cue.rs`. Right-click menus that open below the row
//! they belong to are `row_menu.rs`, and the collapsed Details disclosure under a plain-words
//! message `details.rs`. The tip row (an accent wash with a sentence, its button and a × menu)
//! is `hint_row.rs`, and a row of underline tabs `underline_tab.rs`.

pub mod banner;
mod button_look;
pub mod button_split;
pub mod buttons;
pub mod chip;
pub mod details;
pub mod empty_state;
pub mod field_group;
pub mod form_message;
pub mod ghost;
pub mod hint_row;
pub mod icon;
mod icon_snap;
pub mod menu;
pub mod menu_columns;
pub mod menu_name;
pub mod menu_open;
pub mod menu_reason;
pub mod menu_row;
pub mod menu_split;
pub mod menu_submenu;
mod menu_submenu_keyed;
pub mod quiet_target;
pub mod reason;
pub mod row_fill;
pub mod row_menu;
pub mod scroll_cue;
pub mod section_header;
pub mod segment_bar;
pub mod segmented;
pub mod severity;
pub mod tab_glyph;
pub mod text_field;
mod tool;
pub mod tooltip;
pub mod tooltip_motion;
pub mod truncate;
pub mod underline_tab;

pub use super::list_row::{
    GROUP_ROW_H, GUTTER_X, LIST_ROW_H, NESTED_GUTTER_X, NESTED_INDENT, PAD_X, Part, ROW_INDENT, SECTION_H, TITLE_X, file_row,
    file_row_focus, file_row_with_end, keyed_file_row, keyed_file_row_with_end, list_row, list_row_cut, list_row_hidden, row_text,
    row_text_hover, row_text_reserving,
};
pub use chip::quiet_chip;
pub use field_group::FieldGroup;
pub use icon::{Icon, paint_icon, paint_icon_sized};
pub use section_header::section_fonts;
#[cfg(test)]
pub use section_header::section_header;
pub use text_field::TextField;
pub use tool::*;
pub use truncate::{Cut, cut_label, paint_cut, paint_cut_text, truncate_label};

use super::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::type_scale;

use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Response, Sense, Stroke, Ui, pos2, vec2};

pub fn chevron(p: &Painter, c: Pos2, open: bool, col: Color32) {
    let pts = if open {
        vec![pos2(c.x - 4.0, c.y - 2.0), pos2(c.x + 4.0, c.y - 2.0), pos2(c.x, c.y + 3.0)]
    } else {
        vec![pos2(c.x - 2.0, c.y - 4.0), pos2(c.x + 3.0, c.y), pos2(c.x - 2.0, c.y + 4.0)]
    };
    p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
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

/// Preferences navigation row: icon + label in body text, lit through `row_fill` (the selected
/// page with its accent bar, the focused row with the inset ring).
pub fn nav_row(ui: &mut Ui, icon: Option<Icon>, text: &str, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), type_scale::BODY + 20.0), Sense::click());
    // Painted text has no accessible name; give the row one (screen readers and UI tests), and
    // say which is the current page.
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    if selected {
        crate::ui::access::mark_selected(&resp);
    }
    let p = ui.painter();
    // The selected page takes the accent bar; the focused row the inset ring on its square
    // corners, painted by `row_fill` with the rest of its state.
    row_fill::paint_state(p, rect, row_fill::RowState::of(&resp, selected, resp.hovered()));
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
    ui.add(buttons::icon_only(icon, tooltip).size(ControlSize::Small).enabled(ui.is_enabled()).why(Some(tooltip)))
}

/// A small clickable painted icon with a tooltip, for icons the fonts lack (star, cross): a
/// Small quiet icon-only target (`ControlSize::Small`, 24 x 24, its size's icon in `col`), lit
/// over its whole square on hover and ringed while focused, named and tipped `tooltip`.
pub fn small_icon(ui: &mut Ui, icon: Icon, tooltip: &str, col: Color32) -> Response {
    quiet_icon(ui, icon, tooltip, |_| col)
}

/// A small weak cross that removes something (a key in Preferences > Keyboard, a toast): a
/// Small quiet icon-only target, weak at rest and strong on the hover layer when hovered or
/// focused, named and tipped `name`.
pub fn remove_icon(ui: &mut Ui, name: &str) -> Response {
    quiet_icon(ui, Icon::Close, name, |lit| if lit { color::strong() } else { color::weak() })
}

/// A Small quiet icon-only target showing `icon` in `ink(lit)` (lit: hovered, held or focused),
/// named and tipped `name`.
fn quiet_icon(ui: &mut Ui, icon: Icon, name: &str, ink: impl Fn(bool) -> Color32) -> Response {
    let size = ControlSize::Small;
    let m = size.metrics();
    let (rect, resp) = ui.allocate_exact_size(vec2(m.h, m.h), Sense::click());
    super::access::name_button(&resp, true, name);
    let state = quiet_target::State::of(&resp, false);
    let p = ui.painter();
    quiet_target::paint(p, rect, CornerRadius::same(m.radius), size, state);
    paint_icon_sized(p, rect.center(), icon, ink(state.lit() || state.focused), m.icon);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(name)
}

#[cfg(test)]
#[path = "../snapshot_tests/snapshot_tests_design.rs"]
mod snapshot_tests_design;
