//! Preferences > UI Customization > New group color: which colour a new start-page group takes
//! (`GroupColourDefault`), as a combo of Next unused color, None and the eight hues, each hue
//! with its swatch at the left (the closed combo's too, where the other combos' text starts); Reset all group colors in a group's
//! menu applies it to the groups already made.

use super::{help, row};
use crate::state::repo_colours::hue_name;
use crate::ui::repo_swatch;
use crate::ui::theme::color;
use crate::ui::widgets::menu_columns;
use eframe::egui::{self, Color32, FontId, Rect, Ui, pos2, text::LayoutJob, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{GroupColourDefault, RepoColour, Settings};

/// The room before an option's name: a menu's icon column and the gap after it, as the Color
/// submenu lays out its swatches.
const SWATCH_ROOM: f32 = menu_columns::ICON_W + 8.0;

/// The New group color row.
pub fn group_colour_row(ui: &mut Ui, t: Lang, s: &mut Settings) {
    let label = t.tr("prefs.ui.group_colour.label");
    let text = t.tr("prefs.ui.group_colour.help");
    row(ui, label, |ui| {
        let value = &mut s.new_group_colour;
        let current = *value;
        let r = egui::ComboBox::from_id_salt("group_colour")
            .selected_text(closed_text(button_font(ui), t, current))
            .width(super::fit(ui, 230.0))
            // Every choice at once: ten rows, no scrolling.
            .height(f32::INFINITY)
            .show_ui(ui, |ui| {
                for v in [GroupColourDefault::Next, GroupColourDefault::None] {
                    option(ui, value, v, name(t, v));
                }
                ui.separator();
                for c in RepoColour::ALL {
                    option(ui, value, GroupColourDefault::Hue(c), name(t, GroupColourDefault::Hue(c)));
                }
            });
        if let GroupColourDefault::Hue(c) = current {
            // The swatch's left edge where the other combos' text starts.
            let at = pos2(r.response.rect.left() + ui.spacing().button_padding.x, r.response.rect.center().y);
            repo_swatch::paint(ui.painter(), at, repo_swatch::SIZE, color::repo_hue(c));
        }
        super::accessibility::name(&r.response, label, text);
        help(ui, text);
    });
}

/// The name shown for `v`.
fn name(t: Lang, v: GroupColourDefault) -> &'static str {
    match v {
        GroupColourDefault::Next => t.tr("prefs.ui.group_colour.next"),
        GroupColourDefault::None => t.tr("repo_colour.none"),
        GroupColourDefault::Hue(c) => hue_name(t, c),
    }
}

/// The closed combo's text: a hue's name after its swatch's room, else the name at the usual
/// inset, lined up with the other combos'.
fn closed_text(font: FontId, t: Lang, v: GroupColourDefault) -> egui::WidgetText {
    match v {
        GroupColourDefault::Hue(_) => option_text(font, name(t, v)).into(),
        _ => name(t, v).into(),
    }
}

/// `name` after the swatch's room, in the widget's own text colour.
fn option_text(font: FontId, name: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(name, SWATCH_ROOM, egui::TextFormat::simple(font, Color32::PLACEHOLDER));
    job
}

/// The font a combo and its choices draw their text in.
fn button_font(ui: &Ui) -> FontId {
    egui::TextStyle::Button.resolve(ui.style())
}

/// One choice in the open combo, its swatch in the room before its name.
fn option(ui: &mut Ui, value: &mut GroupColourDefault, v: GroupColourDefault, name: &str) {
    let r = ui.selectable_value(value, v, option_text(button_font(ui), name));
    // Next unused color and None keep the slot empty, so every name lines up.
    if let GroupColourDefault::Hue(c) = v {
        let left = r.rect.left_top() + vec2(ui.spacing().button_padding.x, 0.0);
        let slot = Rect::from_min_size(left, vec2(menu_columns::ICON_W, r.rect.height()));
        repo_swatch::paint_slot(ui.painter(), slot, color::repo_hue(c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The room a closed combo's text leaves before its name (0 for plain text).
    fn lead(text: egui::WidgetText) -> f32 {
        match text {
            egui::WidgetText::LayoutJob(job) => job.sections[0].leading_space,
            _ => 0.0,
        }
    }

    #[test]
    fn the_closed_combo_keeps_the_swatch_room_only_for_a_hue() {
        let font = FontId::proportional(14.0);
        for v in [GroupColourDefault::Next, GroupColourDefault::None] {
            assert_eq!(lead(closed_text(font.clone(), Lang::En, v)), 0.0, "{v:?} starts where the other combos' text does");
        }
        assert_eq!(lead(closed_text(font, Lang::En, GroupColourDefault::Hue(RepoColour::Teal))), SWATCH_ROOM);
    }
}
