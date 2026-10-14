//! Right panel for several selected commits: the list, actions on all of them, and with exactly
//! two, the files that differ between them.

use super::common::short;
use super::details::changed_file_row;
use super::menus::{cherry_pick_button, copy_shas};
use super::theme::color;
use crate::msg::Msg;
use crate::state::{AppState, Selection};
use eframe::egui::{self, RichText};
use trellis_core::repo::DiffTarget;

pub(super) fn panel(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(snap), Selection::Commits(m)) = (&st.snap, &st.selection) else { return };
    ui.label(RichText::new(format!("{} commits selected", m.len())).strong().size(15.0).color(color::text()));
    ui.horizontal_wrapped(|ui| {
        cherry_pick_button(ui, m, msgs);
        if ui.button("Copy SHAs").clicked() {
            msgs.push(copy_shas(m));
        }
        ui.label(RichText::new("Esc returns to one commit").small().color(color::weak()));
    });
    ui.add_space(6.0);

    let list_h = if m.pair().is_some() { 160.0 } else { ui.available_height() };
    egui::ScrollArea::vertical().id_salt("multi-commits").max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
        for &oid in &m.oids {
            let Some(&row) = snap.row_of.get(&oid) else { continue };
            ui.horizontal(|ui| {
                ui.label(RichText::new(short(oid)).monospace().color(color::weak()));
                if ui.selectable_label(false, &snap.commits[row].summary).clicked() {
                    msgs.push(Msg::SelectCommit(oid));
                }
            });
        }
    });

    let Some((older, newer)) = m.pair() else {
        ui.add_space(8.0);
        let note = "File changes are shown when exactly two commits are selected. Ctrl-click a commit to remove it from the selection.";
        ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
        return;
    };
    ui.separator();
    ui.label(RichText::new(format!("Changes from {} to {}", short(older), short(newer))).strong().color(color::text()));
    match &m.changes {
        None => {
            ui.label(RichText::new("Loading…").color(color::weak()));
        }
        Some(Err(e)) => {
            ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
        }
        Some(Ok(files)) if files.is_empty() => {
            ui.label(RichText::new("These commits have the same files.").color(color::weak()));
        }
        Some(Ok(files)) => {
            let open = st.diff.as_ref().map(|(t, _)| t);
            egui::ScrollArea::vertical().id_salt("multi-files").auto_shrink([false, false]).show(ui, |ui| {
                for f in files {
                    let target = DiffTarget::Between(older, newer, f.path.clone());
                    changed_file_row(ui, f, target, open, newer, msgs);
                }
            });
        }
    }
}
