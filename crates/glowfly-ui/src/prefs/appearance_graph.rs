//! Preferences > Appearance > Commit graph: which columns the graph shows (one labelled
//! checkbox each, in one row), the row density, how commit descriptions show, the two layout
//! resets, and the graph's extras: ghost refs, highlighted ref rows, worktree WIP rows and which
//! branches show. The branch colour preset is on Accessibility (Colors).

use super::{check_row, combo, help, row, section};
use crate::ui::theme::controls;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::widgets::buttons;
use crate::ui::widgets::segmented::segmented;
use eframe::egui::Ui;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{BranchVisibility as BV, ColumnWidths, Density, DescriptionMode as DM, GraphColumns, Settings};

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
        controls::checkbox(ui, &mut c.author, t.tr("prefs.ui.graph.author.check"));
        controls::checkbox(ui, &mut c.commit, t.tr("prefs.ui.graph.commit.check"));
        controls::checkbox(ui, &mut c.date, t.tr("prefs.ui.graph.date.check"));
    });
    row(ui, t.tr("prefs.ui.graph.density.label"), |ui| {
        let options = [
            (Density::Comfortable, t.tr("prefs.ui.graph.density.comfortable"), t.tr("prefs.ui.graph.density.comfortable.tip")),
            (Density::Compact, t.tr("prefs.ui.graph.density.compact"), t.tr("prefs.ui.graph.density.compact.tip")),
        ];
        if let Some(d) = segmented(ui, ControlSize::Regular, s.graph_density, &options) {
            s.graph_density = d;
        }
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
    extras(ui, t, s);
}

/// Ghost refs, highlighted ref rows, worktree WIP rows and which branches show.
fn extras(ui: &mut Ui, t: Lang, s: &mut Settings) {
    let (label, check, text) = (t.tr("prefs.ui.ghost.label"), t.tr("prefs.ui.ghost.check"), t.tr("prefs.ui.ghost.help"));
    check_row(ui, label, &mut s.ghost_refs, check, Some(text));
    check_row(ui, t.tr("prefs.ui.highlight_rows.label"), &mut s.highlight_ref_rows, t.tr("prefs.ui.highlight_rows.check"), None);
    let (label, check, text) =
        (t.tr("prefs.ui.worktree_wips.label"), t.tr("prefs.ui.worktree_wips.check"), t.tr("prefs.ui.worktree_wips.help"));
    check_row(ui, label, &mut s.worktree_wips, check, Some(text));
    row(ui, t.tr("prefs.ui.branch_visibility.label"), |ui| {
        let options = [(BV::All, t.tr("prefs.ui.branch_visibility.all")), (BV::Smart, t.tr("prefs.ui.branch_visibility.smart"))];
        combo(ui, "visibility", &mut s.branch_visibility, &options);
        help(ui, t.tr("prefs.ui.branch_visibility.help"));
    });
}
