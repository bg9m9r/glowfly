//! Right panel for several selected commits: the list, actions on all of them, and with exactly
//! two, the files that differ between them and their file actions.

use super::common::{short, widget_row_height};
use super::details::{RowActions, changed_file_row};
use super::file_menu::RowSource;
use super::menus::{cherry_pick_button, copy_shas};
use super::theme::color;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::lfs::TrackContext;
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
    // Only the rows in view are drawn: a range selection can span the whole history.
    let rows: Vec<(git2::Oid, usize)> = m.oids.iter().filter_map(|&o| Some((o, *snap.row_of.get(&o)?))).collect();
    let row_h = widget_row_height(ui);
    egui::ScrollArea::vertical().id_salt("multi-commits").max_height(list_h).auto_shrink([false, true]).show_rows(
        ui,
        row_h,
        rows.len(),
        |ui, range| {
            for &(oid, row) in &rows[range] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(short(oid)).monospace().color(color::weak()));
                    if ui.selectable_label(false, &snap.commits[row].summary).clicked() {
                        msgs.push(Msg::Repo(RepoMsg::SelectCommit(oid)));
                    }
                });
            }
        },
    );

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
        Some(Ok(c)) if c.files.is_empty() => {
            ui.label(RichText::new("These commits have the same files.").color(color::weak()));
        }
        Some(Ok(c)) => {
            let open = st.diff.as_ref().map(|(t, _)| t);
            let lfs = TrackContext::of(st);
            let diff_blocked = crate::state::tool_compare::diff_blocked(&st.settings.external_tools);
            let row_h = widget_row_height(ui);
            egui::ScrollArea::vertical().id_salt("multi-files").auto_shrink([false, false]).show_rows(
                ui,
                row_h,
                c.files.len(),
                |ui, range| {
                    for f in &c.files[range] {
                        let target = DiffTarget::Between(older, newer, f.path.clone());
                        let src = RowSource::Between { older, newer };
                        let actions = Some(RowActions { src, in_workdir: c.in_workdir(&f.path), lfs, diff_blocked });
                        changed_file_row(ui, f, target, open, newer, actions, msgs);
                    }
                },
            );
        }
    }
}
