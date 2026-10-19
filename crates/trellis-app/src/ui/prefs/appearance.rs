//! Preferences > UI customization.

use super::{LABEL_W, help, row};
use crate::msg::{Msg, PrefsMsg};
use crate::themes::ThemeState;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Sense, Stroke, Ui, vec2};
use trellis_core::i18n::Lang;
use trellis_core::settings::Settings;

/// A titled section separator, such as the "Date/time" heading.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(type_scale::section_text(ui, title));
    let (r, _) = ui.allocate_exact_size(vec2(LABEL_W + 520.0, 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0_f32, color::border()));
    ui.add_space(10.0);
}

pub fn combo<T: PartialEq + Copy>(ui: &mut Ui, id: &str, value: &mut T, options: &[(T, &str)]) {
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| *l).unwrap_or("");
    egui::ComboBox::from_id_salt(id).selected_text(current).width(230.0).show_ui(ui, |ui| {
        for (v, label) in options {
            ui.selectable_value(value, *v, *label);
        }
    });
}

/// A date pattern field with a live preview of the current time.
pub fn date_pattern(ui: &mut Ui, label: &str, value: &mut String, default: &str, locale: &str, now: (i64, i32)) {
    row(ui, label, |ui| {
        ui.horizontal(|ui| {
            controls::field(ui, egui::TextEdit::singleline(value).hint_text(default).desired_width(230.0).margin(vec2(8.0, 5.0)));
            let pattern = if value.trim().is_empty() { default } else { value.as_str() };
            let preview = trellis_core::dates::format(now.0, pattern, locale, now.1);
            ui.label(RichText::new(preview).size(type_scale::SMALL).color(color::weak()));
        });
    });
}

pub fn ui_page(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, now: (i64, i32), msgs: &mut Vec<Msg>) {
    use trellis_core::settings::Corner;
    let t = s.lang();
    super::theme_row::theme_row(ui, s, themes, msgs);
    row(ui, t.tr("prefs.ui.notifications.label"), |ui| {
        combo(
            ui,
            "toasts",
            &mut s.notification_location,
            &[
                (Corner::BottomLeft, t.tr("prefs.ui.corner.bottom_left")),
                (Corner::BottomRight, t.tr("prefs.ui.corner.bottom_right")),
                (Corner::TopLeft, t.tr("prefs.ui.corner.top_left")),
                (Corner::TopRight, t.tr("prefs.ui.corner.top_right")),
            ],
        )
    });
    row(ui, t.tr("prefs.ui.behind_alerts.label"), |ui| {
        controls::checkbox(ui, &mut s.behind_alerts, "");
        help(ui, t.tr("prefs.ui.behind_alerts.help"));
    });
    row(ui, t.tr("prefs.ui.desktop_notifications.label"), |ui| {
        // Not bound to the setting: update saves it and sends the confirming notification.
        let mut on = s.desktop_notifications;
        if controls::checkbox(ui, &mut on, t.tr("prefs.ui.desktop_notifications.check")).changed() {
            msgs.push(Msg::Prefs(PrefsMsg::SetDesktopNotifications(on)));
        }
        help(ui, t.tr("prefs.ui.desktop_notifications.help"));
    });
    date_time_section(ui, t, s, now);
    interface_section(ui, t, s);
    super::appearance_graph::graph_section(ui, t, s);
}

/// Date/Time: the locale and the four patterns, with the tokens they take (which stay literal
/// in every language).
fn date_time_section(ui: &mut Ui, t: Lang, s: &mut Settings, now: (i64, i32)) {
    section(ui, t.tr("prefs.ui.date_time.section"));
    ui.horizontal(|ui| {
        ui.label(RichText::new(t.tr("prefs.ui.date_time.patterns_help")).size(type_scale::SMALL).color(color::weak()));
    });
    ui.add_space(10.0);
    row(ui, t.tr("prefs.ui.date_locale.label"), |ui| {
        let current = trellis_core::dates::LOCALES
            .iter()
            .find(|(k, _)| *k == s.date_locale)
            .map(|(_, l)| *l)
            .unwrap_or(s.date_locale.as_str())
            .to_owned();
        let shown = if s.date_locale == "system" {
            t.trf("prefs.ui.date_locale.system", &[("locale", &sys_locale::get_locale().unwrap_or_default())])
        } else {
            current
        };
        egui::ComboBox::from_id_salt("locale").selected_text(shown).width(230.0).show_ui(ui, |ui| {
            for (k, label) in trellis_core::dates::LOCALES {
                ui.selectable_value(&mut s.date_locale, k.to_owned(), label);
            }
        });
    });
    let d = Settings::default();
    let loc = s.date_locale.clone();
    date_pattern(ui, t.tr("prefs.ui.date_format.label"), &mut s.date_format, &d.date_format, &loc, now);
    date_pattern(ui, t.tr("prefs.ui.date_word_format.label"), &mut s.date_word_format, &d.date_word_format, &loc, now);
    date_pattern(ui, t.tr("prefs.ui.date_time_format.label"), &mut s.date_time_format, &d.date_time_format, &loc, now);
    date_pattern(ui, t.tr("prefs.ui.date_verbose_format.label"), &mut s.date_verbose_format, &d.date_verbose_format, &loc, now);
    row(ui, t.tr("prefs.ui.relative_dates.label"), |ui| {
        controls::checkbox(ui, &mut s.relative_dates, t.tr("prefs.ui.relative_dates.check"));
    });
}

/// Interface: zoom, the panels, toolbar labels, reduced motion, spell checking and the graph's
/// extras.
fn interface_section(ui: &mut Ui, t: Lang, s: &mut Settings) {
    use trellis_core::settings::BranchVisibility as BV;
    section(ui, t.tr("prefs.ui.interface.section"));
    row(ui, t.tr("prefs.ui.zoom.label"), |ui| {
        egui::ComboBox::from_id_salt("zoom").selected_text(format!("{}%", s.zoom_level())).width(230.0).show_ui(ui, |ui| {
            for p in trellis_core::settings::ZOOM_STEPS {
                ui.selectable_value(&mut s.zoom_percent, p, format!("{p}%"));
            }
        });
    });
    row(ui, t.tr("prefs.ui.left_panel.label"), |ui| {
        controls::checkbox(ui, &mut s.show_left_panel, "");
    });
    row(ui, t.tr("prefs.ui.details_panel.label"), |ui| {
        controls::checkbox(ui, &mut s.show_details_panel, "");
    });
    row(ui, t.tr("prefs.ui.toolbar.label"), |ui| {
        controls::checkbox(ui, &mut s.show_toolbar, "");
        help(ui, t.tr("prefs.ui.toolbar.help"));
    });
    row(ui, t.tr("prefs.ui.menu_bar.label"), |ui| {
        controls::checkbox(ui, &mut s.show_menu_bar, "");
        help(ui, t.tr("prefs.ui.menu_bar.help"));
    });
    row(ui, t.tr("prefs.ui.toolbar_labels.label"), |ui| {
        controls::checkbox(ui, &mut s.toolbar_labels, "");
    });
    row(ui, t.tr("prefs.ui.reduce_motion.label"), |ui| {
        controls::checkbox(ui, &mut s.reduce_motion, "");
        help(ui, t.tr("prefs.ui.reduce_motion.help"));
    });
    row(ui, t.tr("prefs.ui.spellcheck.label"), |ui| {
        controls::checkbox(ui, &mut s.spellcheck, "");
        help(ui, t.tr("prefs.ui.spellcheck.help"));
    });
    row(ui, t.tr("prefs.ui.initials.label"), |ui| {
        controls::checkbox(ui, &mut s.use_initials, "");
        help(ui, t.tr("prefs.ui.initials.help"));
    });
    row(ui, t.tr("prefs.ui.ghost.label"), |ui| {
        controls::checkbox(ui, &mut s.ghost_refs, "");
        help(ui, t.tr("prefs.ui.ghost.help"));
    });
    row(ui, t.tr("prefs.ui.highlight_rows.label"), |ui| {
        controls::checkbox(ui, &mut s.highlight_ref_rows, "");
    });
    row(ui, t.tr("prefs.ui.worktree_wips.label"), |ui| {
        controls::checkbox(ui, &mut s.worktree_wips, "");
        help(ui, t.tr("prefs.ui.worktree_wips.help"));
    });
    row(ui, t.tr("prefs.ui.group_tabs.label"), |ui| {
        controls::checkbox(ui, &mut s.group_worktree_tabs, t.tr("prefs.ui.group_tabs.check"));
        help(ui, t.tr("prefs.ui.group_tabs.help"));
    });
    row(ui, t.tr("prefs.ui.branch_visibility.label"), |ui| {
        let options = [(BV::All, t.tr("prefs.ui.branch_visibility.all")), (BV::Smart, t.tr("prefs.ui.branch_visibility.smart"))];
        combo(ui, "visibility", &mut s.branch_visibility, &options);
        help(ui, t.tr("prefs.ui.branch_visibility.help"));
    });
}
