//! Preferences > UI customization > Commit graph: which columns the graph shows (one labelled
//! checkbox each, in one row), how commit descriptions show, and the two layout resets.

use super::{combo, help, row, section};
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use eframe::egui::Ui;
use trellis_core::i18n::Lang;
use trellis_core::settings::{ColumnWidths, DescriptionMode as DM, GraphColumns, Settings};

pub fn graph_section(ui: &mut Ui, t: Lang, s: &mut Settings) {
    section(ui, t.tr("prefs.ui.graph.section"));
    let c = &mut s.columns;
    // One checkbox per column, left to right as the graph shows them.
    row(ui, t.tr("prefs.ui.graph.columns.label"), |ui| {
        controls::checkbox(ui, &mut c.refs, t.tr("prefs.ui.graph.refs.check"));
        controls::checkbox(ui, &mut c.tree, t.tr("prefs.ui.graph.tree.check"));
        controls::checkbox(ui, &mut c.message, t.tr("prefs.ui.graph.message.check"));
        controls::checkbox(ui, &mut c.changes, t.tr("prefs.ui.graph.changes.check"));
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            help(ui, t.tr("prefs.ui.graph.changes.help"));
        });
        controls::checkbox(ui, &mut c.commit, t.tr("prefs.ui.graph.author.check"));
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            ui.add_enabled_ui(c.commit, |ui| controls::checkbox(ui, &mut c.commit_sha, t.tr("prefs.ui.graph.sha.check")));
        });
        controls::checkbox(ui, &mut c.date, t.tr("prefs.ui.graph.date.check"));
    });
    row(ui, t.tr("prefs.ui.graph.description.label"), |ui| {
        let options = [
            (DM::Always, t.tr("prefs.ui.graph.description.always")),
            (DM::OnHover, t.tr("prefs.ui.graph.description.on_hover")),
            (DM::Never, t.tr("prefs.ui.graph.description.never")),
        ];
        combo(ui, "description", &mut c.description, &options)
    });
    row(ui, t.tr("prefs.ui.columns.label"), |ui| {
        ui.horizontal(|ui| {
            if ui.add(buttons::secondary(t.tr("prefs.ui.columns.default.button"))).clicked() {
                *c = GraphColumns::default();
                s.column_widths = ColumnWidths::default();
            }
            if ui.add(buttons::secondary(t.tr("prefs.ui.columns.compact.button"))).clicked() {
                *c = GraphColumns::compact();
                s.column_widths = ColumnWidths::default();
            }
        });
        help(ui, t.tr("prefs.ui.columns.help"));
    });
}
