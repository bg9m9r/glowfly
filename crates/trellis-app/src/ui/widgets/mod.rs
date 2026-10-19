//! Custom-painted widget controls: toolbar buttons, section headers, chips, nav rows, icon
//! buttons; icon shapes are in `icon.rs`, and text cut to a width with an ellipsis is in
//! `truncate.rs`.

pub mod buttons;
mod icon;
pub mod truncate;

pub use super::list_row::{Part, list_row, list_row_cut, list_row_hidden};
pub use icon::{Icon, paint_icon};
pub use truncate::{Cut, cut_label, paint_cut, paint_cut_text, truncate_label};

use super::theme::color;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

/// Toolbar button: caption above the icon (caption optional), tooltip with
/// the caption when labels are hidden. `name` is its accessible name, which includes the caption.
pub fn tool_button(ui: &mut Ui, icon: Icon, label: &str, name: &str, enabled: bool, labels: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    // A label may overhang the 54 px slot by 2.5 px a side, as English "Commands" does; a longer
    // one (a translation, say) widens the button to fit it with 2 px to spare a side.
    let width = if labels {
        let text_w = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(12.0), Color32::PLACEHOLDER).size().x;
        if text_w > 59.0 { text_w + 4.0 } else { 54.0 }
    } else {
        38.0
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 46.0), sense);
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(4), color::hover());
    }
    let col = if enabled { color::text() } else { color::faint() };
    if labels {
        p.text(
            pos2(rect.center().x, rect.top() + 9.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            if enabled { color::weak() } else { color::faint() },
        );
        paint_icon(p, pos2(rect.center().x, rect.top() + 30.0), icon, col);
    } else {
        paint_icon(p, rect.center(), icon, col);
    }
    super::access::name_button(&resp, enabled, name);
    let resp = if labels { resp } else { resp.on_hover_text(label) };
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Small dropdown caret button that sits next to a tool button.
pub fn caret_button(ui: &mut Ui) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 46.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), color::hover());
    }
    let c = pos2(rect.center().x, rect.top() + 28.0);
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

/// Sidebar section header: chevron, uppercase title, count on the right. Toggles `open`.
pub fn section_header(ui: &mut Ui, title: &str, count: Option<usize>, open: &mut bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 0.0, color::hover());
    }
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0_f32, color::border()));
    chevron(p, pos2(rect.left() + 10.0, rect.center().y), *open, color::weak());
    p.text(pos2(rect.left() + 22.0, rect.center().y), Align2::LEFT_CENTER, title, FontId::proportional(13.0), color::text());
    if let Some(n) = count {
        p.text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, n.to_string(), FontId::proportional(13.0), color::blue());
    }
    if resp.clicked() {
        *open = !*open;
    }
    resp
}

/// Rounded label chip drawn with the painter; returns its width.
pub fn chip(p: &Painter, x: f32, y: f32, text: &str, fill: Color32, fg: Color32) -> f32 {
    let g = p.layout_no_wrap(text.to_owned(), FontId::proportional(11.5), fg);
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

/// Preferences navigation row: icon + label, highlighted when selected.
pub fn nav_row(ui: &mut Ui, icon: Option<Icon>, text: &str, selected: bool, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), size + 20.0), Sense::click());
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
        FontId::proportional(size),
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
    let fill = if enabled && resp.hovered() { color::hover() } else { color::toolbar() };
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
