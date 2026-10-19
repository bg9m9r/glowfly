//! Preferences > UI Customization > Commit Graph: which columns the graph shows, and the two
//! layout resets.

use super::appearance::{combo, section};
use super::{help, row};
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use eframe::egui::Ui;
use trellis_core::i18n::Lang;
use trellis_core::settings::{ColumnWidths, DescriptionMode as DM, GraphColumns, Settings};

pub fn graph_section(ui: &mut Ui, t: Lang, s: &mut Settings) {
    section(ui, t.tr("prefs.ui.graph.section"));
    let c = &mut s.columns;
    row(ui, t.tr("prefs.ui.graph.refs.label"), |ui| {
        controls::checkbox(ui, &mut c.refs, "");
    });
    row(ui, t.tr("prefs.ui.graph.tree.label"), |ui| {
        controls::checkbox(ui, &mut c.tree, "");
    });
    row(ui, t.tr("prefs.ui.graph.sha.label"), |ui| {
        controls::checkbox(ui, &mut c.sha, "");
    });
    row(ui, t.tr("prefs.ui.graph.message.label"), |ui| {
        controls::checkbox(ui, &mut c.message, "");
    });
    row(ui, t.tr("prefs.ui.graph.description.label"), |ui| {
        let options = [
            (DM::Always, t.tr("prefs.ui.graph.description.always")),
            (DM::OnHover, t.tr("prefs.ui.graph.description.on_hover")),
            (DM::Never, t.tr("prefs.ui.graph.description.never")),
        ];
        combo(ui, "description", &mut c.description, &options)
    });
    row(ui, t.tr("prefs.ui.graph.changes.label"), |ui| {
        controls::checkbox(ui, &mut c.changes, "");
        help(ui, t.tr("prefs.ui.graph.changes.help"));
    });
    row(ui, t.tr("prefs.ui.graph.author.label"), |ui| {
        controls::checkbox(ui, &mut c.author, "");
    });
    row(ui, t.tr("prefs.ui.graph.date.label"), |ui| {
        controls::checkbox(ui, &mut c.date, "");
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
