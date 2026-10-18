//! Right panel for one commit compared with the working directory: the commit, and the files
//! that differ from it to the working tree (staged and unstaged changes and untracked files),
//! each opening that diff.

use super::common::{short, widget_row_height};
use super::details::{RowActions, changed_file_row};
use super::file_menu::RowSource;
use super::theme::color;
use crate::msg::Msg;
use crate::state::lfs::TrackContext;
use crate::state::{AppState, Selection};
use eframe::egui::{self, RichText};
use trellis_core::repo::DiffTarget;

pub(super) fn panel(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(snap), Selection::VsWorkdir(w)) = (&st.snap, &st.selection) else { return };
    let oid = w.oid;
    let t = st.settings.lang();
    let header = t.trf("details.workdir.header", &[("sha", &short(oid))]);
    ui.add(egui::Label::new(RichText::new(header).monospace().color(color::weak())).wrap());
    if let Some(&row) = snap.row_of.get(&oid) {
        ui.add(egui::Label::new(RichText::new(&snap.commits[row].summary).strong().size(15.0).color(color::text())).wrap());
    }
    ui.horizontal(|ui| {
        if ui.small_button(t.tr("common.copy_sha.button")).clicked() {
            msgs.push(Msg::Copy(oid.to_string()));
        }
        ui.label(RichText::new(t.tr("details.workdir.esc_hint")).small().color(color::weak()));
    });
    ui.separator();
    ui.label(RichText::new(t.trf("details.workdir.changes", &[("sha", &short(oid))])).strong().color(color::text()));
    match &w.changes {
        None => {
            ui.label(RichText::new(t.tr("common.loading")).color(color::weak()));
        }
        Some(Err(e)) => {
            ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
        }
        Some(Ok(c)) if c.files.is_empty() => {
            ui.label(RichText::new(t.tr("details.workdir.same")).color(color::weak()));
        }
        Some(Ok(c)) => {
            let open = st.diff.as_ref().map(|(target, _)| target);
            let lfs = TrackContext::of(st);
            let diff_blocked = crate::state::tool_compare::diff_blocked(&st.settings.external_tools);
            egui::ScrollArea::vertical().id_salt("workdir-files").auto_shrink([false, false]).show_rows(
                ui,
                widget_row_height(ui),
                c.files.len(),
                |ui, range| {
                    for f in &c.files[range] {
                        let target = DiffTarget::ToWorkdir(oid, f.path.clone());
                        let src = RowSource::ToWorkdir(oid);
                        let in_workdir = c.in_workdir(&f.path);
                        let actions = RowActions { src, history_at: oid, in_workdir, lfs, diff_blocked, patch_blocked: None };
                        changed_file_row(ui, t, f, target, open, actions, msgs);
                    }
                },
            );
        }
    }
}
