//! Preferences > Appearance > Theme: the built-in themes as cards, grouped dark and light,
//! each a small picture of the main window in that theme (title bar, sidebar, graph lanes with
//! main in the accent, a selected row and a primary button) with its name under it and its
//! description on hover. "Match the system" sits above them.

use crate::state::theme_names;
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::tokens::{self, radius};
use crate::ui::theme::{builtin, fonts, type_scale};
use eframe::egui::{self, Align2, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{BuiltinTheme, ThemeChoice};

/// A card's preview height, the name line under it, and the gaps between cards.
const PREVIEW_H: f32 = 84.0;
const NAME_H: f32 = 28.0;
const GAP_X: f32 = 14.0;
const GAP_Y: f32 = 12.0;
/// The narrowest and widest a card gets as the column stretches.
const MIN_W: f32 = 132.0;
const MAX_W: f32 = 176.0;

/// The built-in themes of one kind, in menu order (the default one first).
pub fn group(dark: bool) -> Vec<BuiltinTheme> {
    ThemeChoice::BUILT_IN
        .iter()
        .filter_map(|c| c.builtin(true).filter(|_| *c != ThemeChoice::System))
        .filter(|b| b.is_dark() == dark)
        .collect()
}

/// The Match system check, then the dark and light cards. Sets `theme` on a click.
pub fn picker(ui: &mut Ui, t: Lang, theme: &mut ThemeChoice) {
    let system_dark = ui.ctx().system_theme().is_none_or(|s| s == egui::Theme::Dark);
    let mut system = *theme == ThemeChoice::System;
    if crate::ui::theme::controls::checkbox(ui, &mut system, t.tr("prefs.ui.theme.system.check")).changed() {
        // Unticked, the theme stays the one on screen.
        *theme = if system { ThemeChoice::System } else { theme.builtin(system_dark).map_or(ThemeChoice::Graphite, BuiltinTheme::choice) };
    }
    ui.add_space(10.0);
    let shown = theme.builtin(system_dark);
    for (dark, key) in [(true, "prefs.ui.theme.group.dark"), (false, "prefs.ui.theme.group.light")] {
        ui.label(egui::RichText::new(t.tr(key)).font(fonts::semibold(ui, type_scale::SMALL)).color(color::weak()));
        ui.add_space(6.0);
        if let Some(b) = cards(ui, t, &group(dark), shown) {
            *theme = b.choice();
        }
        ui.add_space(8.0);
    }
    if let Some(b) = shown {
        let line = format!("{}: {}", theme_names::builtin(t, b), theme_names::hint(t, b));
        ui.add(egui::Label::new(egui::RichText::new(line).size(type_scale::SMALL).color(color::weak())).wrap());
    }
}

/// One group's cards, as many to a row as fit; the theme clicked, if any.
fn cards(ui: &mut Ui, t: Lang, themes: &[BuiltinTheme], shown: Option<BuiltinTheme>) -> Option<BuiltinTheme> {
    let avail = ui.available_width();
    let cols = (((avail + GAP_X) / (MIN_W + GAP_X)).floor() as usize).clamp(1, themes.len().max(1));
    let w = ((avail - GAP_X * (cols as f32 - 1.0)) / cols as f32).clamp(MIN_W.min(avail), MAX_W);
    let mut picked = None;
    for row in themes.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP_X;
            for &b in row {
                if card(ui, t, b, shown == Some(b), w).clicked() {
                    picked = Some(b);
                }
            }
        });
        ui.add_space(GAP_Y - ui.spacing().item_spacing.y);
    }
    picked
}

fn card(ui: &mut Ui, t: Lang, b: BuiltinTheme, selected: bool, w: f32) -> Response {
    let name = theme_names::builtin(t, b);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, PREVIEW_H + NAME_H), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, name));
    if ui.is_rect_visible(rect) {
        let preview = Rect::from_min_size(rect.min, vec2(w, PREVIEW_H));
        let painter = ui.painter();
        paint_preview(painter, preview, builtin(b));
        let edge = if selected {
            Stroke::new(tokens::FOCUS_W, color::accent())
        } else if resp.hovered() {
            Stroke::new(1.0_f32, color::input_border())
        } else {
            Stroke::new(1.0_f32, color::border())
        };
        painter.rect_stroke(preview, radius::LG, edge, StrokeKind::Outside);
        if resp.has_focus() {
            tokens::focus_ring(painter, preview.expand(if selected { tokens::FOCUS_W } else { 0.0 }), radius::LG);
        }
        let font = if selected { fonts::semibold(ui, type_scale::BODY) } else { type_scale::font(type_scale::BODY) };
        let ink = if selected { color::strong() } else { color::text() };
        painter.text(pos2(rect.left() + 2.0, preview.bottom() + 8.0), Align2::LEFT_TOP, name, font, ink);
    }
    resp.on_hover_text(theme_names::hint(t, b))
}

/// The main window in `p`, in miniature: title bar with the active tab, the sidebar with three
/// branches, the graph with main in the accent and two lanes, a selected row and a primary
/// button.
fn paint_preview(painter: &egui::Painter, r: Rect, p: &Palette) {
    let rad = radius::LG;
    painter.rect_filled(r, rad, p.bg);
    // Title bar, with the active tab in the panel colour and the accent's line under it.
    let bar = Rect::from_min_size(r.min, vec2(r.width(), 14.0));
    painter.rect_filled(bar, radius::top(rad), p.toolbar);
    let tab = Rect::from_min_size(pos2(r.left() + 8.0, bar.top() + 3.0), vec2(30.0, 11.0));
    painter.rect_filled(tab, radius::top(radius::SM), p.panel);
    painter.rect_filled(Rect::from_min_size(tab.left_top(), vec2(tab.width(), 2.0)), CornerRadius::ZERO, p.accent);
    // Sidebar: the panel, three branches as a dot and a name.
    let side = Rect::from_min_max(pos2(r.left(), bar.bottom()), pos2(r.left() + (r.width() * 0.27).round(), r.bottom()));
    painter.rect_filled(side, CornerRadius { sw: rad, ..CornerRadius::ZERO }, p.panel);
    for (i, c) in [p.accent, p.branch.lanes[4], p.branch.lanes[0]].into_iter().enumerate() {
        let y = side.top() + 12.0 + i as f32 * 13.0;
        painter.circle_filled(pos2(side.left() + 8.0, y), 2.5, c);
        let len: f32 = [16.0, 20.0, 13.0][i];
        bar_at(painter, pos2(side.left() + 14.0, y), len.min(side.width() - 18.0), if i == 0 { p.text } else { p.weak });
    }
    // The graph: rows, the selected one tinted with the accent's bar at its left edge.
    let main = Rect::from_min_max(pos2(side.right(), bar.bottom()), r.max);
    let rows: Vec<f32> = (0..4).map(|i| main.top() + 12.0 + i as f32 * 15.0).collect();
    let sel = Rect::from_center_size(pos2(main.center().x, rows[1]), vec2(main.width(), 13.0));
    painter.rect_filled(sel, CornerRadius::ZERO, p.selected);
    painter.rect_filled(Rect::from_min_size(sel.min, vec2(2.0, sel.height())), CornerRadius::ZERO, p.accent);
    let (x0, x1, x2) = (main.left() + 10.0, main.left() + 19.0, main.left() + 28.0);
    let stroke = |c| Stroke::new(1.5_f32, c);
    painter.line_segment([pos2(x1, rows[0]), pos2(x1, rows[2])], stroke(p.branch.lanes[4]));
    painter.line_segment([pos2(x1, rows[2]), pos2(x0, rows[3])], stroke(p.branch.lanes[4]));
    painter.line_segment([pos2(x2, rows[1]), pos2(x0, rows[2])], stroke(p.branch.lanes[0]));
    painter.line_segment([pos2(x0, rows[0] - 6.0), pos2(x0, main.bottom() - 2.0)], stroke(p.accent));
    let dots = [(x1, p.branch.lanes[4]), (x2, p.branch.lanes[0]), (x0, p.accent), (x0, p.accent)];
    let text_x = x2 + 9.0;
    for (i, (&y, (x, c))) in rows.iter().zip(dots).enumerate() {
        painter.circle_filled(pos2(x, y), 3.0, c);
        let ink = if i == 1 { p.strong } else { p.text };
        let room = (main.right() - text_x - 8.0).max(0.0);
        bar_at(painter, pos2(text_x, y), (room * [0.62, 0.5, 0.7, 0.42][i]).round(), ink);
    }
    // A primary button in the bottom corner.
    let button = Rect::from_min_size(pos2(main.right() - 34.0, main.bottom() - 13.0), vec2(26.0, 8.0));
    painter.rect_filled(button, radius::pill(button.height()), p.primary);
}

/// A line of text as a rounded bar starting at `at` (its vertical centre).
fn bar_at(painter: &egui::Painter, at: egui::Pos2, len: f32, c: egui::Color32) {
    if len > 0.0 {
        painter.rect_filled(Rect::from_min_size(pos2(at.x, at.y - 1.5), vec2(len, 3.0)), radius::pill(3.0), c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_groups_hold_every_builtin_once() {
        let (dark, light) = (group(true), group(false));
        assert_eq!((dark[0], light[0]), (BuiltinTheme::Graphite, BuiltinTheme::Porcelain), "the default pair leads");
        // Each light twin sits under its dark one.
        assert_eq!(dark, [BuiltinTheme::Graphite, BuiltinTheme::Midnight, BuiltinTheme::Dark, BuiltinTheme::HighContrastDark]);
        assert_eq!(light, [BuiltinTheme::Porcelain, BuiltinTheme::Mist, BuiltinTheme::Light, BuiltinTheme::HighContrastLight]);
        assert_eq!(dark.len() + light.len(), BuiltinTheme::ALL.len());
        for b in BuiltinTheme::ALL {
            assert!(dark.contains(&b) != light.contains(&b), "{b:?}");
        }
    }
}
