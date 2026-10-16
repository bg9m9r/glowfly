//! Preferences > UI Customization.

use super::{LABEL_W, help, row};
use crate::ui::theme::color;
use eframe::egui::{self, RichText, Sense, Stroke, Ui, vec2};
use trellis_core::settings::Settings;

/// A titled section separator, such as the "Date/Time" heading.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(15.0).color(color::text()));
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
            ui.add(egui::TextEdit::singleline(value).hint_text(default).desired_width(230.0).margin(vec2(8.0, 5.0)));
            let pattern = if value.trim().is_empty() { default } else { value.as_str() };
            let preview = trellis_core::dates::format(now.0, pattern, locale, now.1);
            ui.label(RichText::new(preview).size(12.0).color(color::weak()));
        });
    });
}

pub fn ui_page(ui: &mut Ui, s: &mut Settings, now: (i64, i32)) {
    use trellis_core::settings::{BranchVisibility as BV, ColumnWidths, Corner, DescriptionMode as DM, GraphColumns, ThemeChoice as TC};
    row(ui, "Theme", |ui| {
        combo(ui, "theme", &mut s.theme, &[(TC::Dark, "Trellis Dark"), (TC::Light, "Trellis Light"), (TC::System, "Match system")])
    });
    row(ui, "Notification Location", |ui| {
        combo(
            ui,
            "toasts",
            &mut s.notification_location,
            &[
                (Corner::BottomLeft, "Bottom Left"),
                (Corner::BottomRight, "Bottom Right"),
                (Corner::TopLeft, "Top Left"),
                (Corner::TopRight, "Top Right"),
            ],
        )
    });
    row(ui, "Behind alerts", |ui| {
        ui.checkbox(&mut s.behind_alerts, "");
        help(ui, "After a background fetch, say when the checked-out branch has fallen behind its upstream, with a button to pull.");
    });

    section(ui, "Date/Time");
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Patterns use moment.js tokens: YYYY, MM, MMMM, DD, D, dddd, HH, h, mm, ss, A. Text in [brackets] is literal.")
                .size(12.0)
                .color(color::weak()),
        );
    });
    ui.add_space(10.0);
    row(ui, "Date/Time Locale", |ui| {
        let current = trellis_core::dates::LOCALES
            .iter()
            .find(|(k, _)| *k == s.date_locale)
            .map(|(_, l)| *l)
            .unwrap_or(s.date_locale.as_str())
            .to_owned();
        let shown = if s.date_locale == "system" { format!("System [{}]", sys_locale::get_locale().unwrap_or_default()) } else { current };
        egui::ComboBox::from_id_salt("locale").selected_text(shown).width(230.0).show_ui(ui, |ui| {
            for (k, label) in trellis_core::dates::LOCALES {
                ui.selectable_value(&mut s.date_locale, k.to_owned(), label);
            }
        });
    });
    let d = Settings::default();
    let loc = s.date_locale.clone();
    date_pattern(ui, "Date Format", &mut s.date_format, &d.date_format, &loc, now);
    date_pattern(ui, "Date word Format", &mut s.date_word_format, &d.date_word_format, &loc, now);
    date_pattern(ui, "Date/Time Format", &mut s.date_time_format, &d.date_time_format, &loc, now);
    date_pattern(ui, "Date verbose Format", &mut s.date_verbose_format, &d.date_verbose_format, &loc, now);
    row(ui, "Relative Dates in Graph", |ui| {
        ui.checkbox(&mut s.relative_dates, "Show \"3 hours ago\" instead of the Date/Time format");
    });

    section(ui, "Interface");
    row(ui, "Show toolbar icon labels", |ui| {
        ui.checkbox(&mut s.toolbar_labels, "");
    });
    row(ui, "Enable spell checking", |ui| {
        ui.checkbox(&mut s.spellcheck, "");
        help(
            ui,
            "Underlines misspellings in the commit summary and description; right-click for suggestions. Uses the Windows spell checker in your system language.",
        );
    });
    row(ui, "Display author initials instead of avatars", |ui| {
        ui.checkbox(&mut s.use_initials, "");
        help(
            ui,
            "Avatars come from GitHub for noreply addresses and from Gravatar otherwise (looked up by a hash of the email). They are cached on disk.",
        );
    });
    row(ui, "Show ghost branch on the current commit", |ui| {
        ui.checkbox(&mut s.ghost_refs, "");
        help(
            ui,
            "Shows a faded \"+ branch\" label after the checked-out commit's branches when you hover or select it; click it to branch from the current branch. To branch from another commit, right-click it.",
        );
    });
    row(ui, "Highlight associated rows when hovering over a branch", |ui| {
        ui.checkbox(&mut s.highlight_ref_rows, "");
    });
    row(ui, "Branch visibility in commit graph", |ui| {
        combo(ui, "visibility", &mut s.branch_visibility, &[(BV::All, "All"), (BV::Smart, "Smart")]);
        help(ui, "Smart shows the checked-out branch, its upstream, the default branch, and branches with open pull requests.");
    });

    section(ui, "Commit Graph");
    let c = &mut s.columns;
    row(ui, "Show branches and tags in graph", |ui| {
        ui.checkbox(&mut c.refs, "");
    });
    row(ui, "Show commit tree in graph", |ui| {
        ui.checkbox(&mut c.tree, "");
    });
    row(ui, "Show commit sha in graph", |ui| {
        ui.checkbox(&mut c.sha, "");
    });
    row(ui, "Show commit message in graph", |ui| {
        ui.checkbox(&mut c.message, "");
    });
    row(ui, "Show commit description in graph", |ui| {
        combo(ui, "description", &mut c.description, &[(DM::Always, "Always"), (DM::OnHover, "On hover"), (DM::Never, "Never")])
    });
    row(ui, "Show commit changes in graph", |ui| {
        ui.checkbox(&mut c.changes, "");
        help(ui, "Lines added and removed per commit, computed in the background.");
    });
    row(ui, "Show commit author in graph", |ui| {
        ui.checkbox(&mut c.author, "");
    });
    row(ui, "Show commit date/time in graph", |ui| {
        ui.checkbox(&mut c.date, "");
    });
    row(ui, "Graph Columns Layout", |ui| {
        ui.horizontal(|ui| {
            if ui.button("Reset to default layout").clicked() {
                *c = GraphColumns::default();
                s.column_widths = ColumnWidths::default();
            }
            if ui.button("Reset to compact layout").clicked() {
                *c = GraphColumns::compact();
                s.column_widths = ColumnWidths::default();
            }
        });
        help(
            ui,
            "Either reset also puts every column back to its standard width. To resize a column, drag the divider beside its title in the graph header; double-click a divider to reset just that column.",
        );
    });
}
