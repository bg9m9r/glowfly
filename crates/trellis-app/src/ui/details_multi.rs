//! Right panel for several selected commits: the list, actions on all of them, and for a pair or
//! one unbroken line of commits, the files that differ across them (either way round) and their
//! file actions; otherwise why there is no combined diff.

use super::common::short;
use super::details::file_reveal::reveal;
use super::details::{RowActions, changed_file_row};
use super::file_menu::RowSource;
use super::menus::{cherry_pick_button, copy_shas, patch_button, revert_button};
use super::theme::color;
use super::widgets;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::state::lfs::TrackContext;
use crate::state::selection::MultiSelection;
use crate::state::selection_span::Span;
use crate::state::{AppState, Selection};
use eframe::egui::{self, RichText};
use trellis_core::i18n::Lang;
use trellis_core::repo::DiffTarget;

pub(super) fn panel(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(snap), Selection::Commits(m)) = (&st.snap, &st.selection) else { return };
    let t = st.settings.lang();
    ui.label(RichText::new(t.trn("details.multi.selected", m.len(), &[])).strong().size(15.0).color(color::text()));
    ui.horizontal_wrapped(|ui| {
        cherry_pick_button(ui, t.trn("details.multi.cherry_pick.button", m.len(), &[]), m, msgs);
        revert_button(ui, t.trn("details.multi.revert.button", m.len(), &[]), m, msgs);
        patch_button(ui, t.trn("details.multi.patch.button", m.len(), &[]), m, msgs);
        if ui.button(t.tr("details.multi.copy_shas.button")).clicked() {
            msgs.push(copy_shas(m));
        }
        ui.label(RichText::new(t.tr("details.multi.esc_hint")).small().color(color::weak()));
    });
    ui.add_space(6.0);

    let list_h = if m.compare_ends().is_some() { 160.0 } else { ui.available_height() };
    // Only the rows in view are drawn: a range selection can span the whole history.
    let rows: Vec<(git2::Oid, usize)> = m.oids.iter().filter_map(|&o| Some((o, *snap.row_of.get(&o)?))).collect();
    let row_h = widgets::LIST_ROW_H;
    egui::ScrollArea::vertical().id_salt("multi-commits").max_height(list_h).auto_shrink([false, true]).show_rows(
        ui,
        row_h,
        rows.len(),
        |ui, range| {
            for &(oid, row) in &rows[range] {
                widgets::file_row(ui, |ui| {
                    ui.label(RichText::new(short(oid)).monospace().color(color::weak()));
                    if ui.selectable_label(false, &snap.commits[row].summary).clicked() {
                        msgs.push(Msg::Repo(RepoMsg::SelectCommit(oid)));
                    }
                });
            }
        },
    );

    let Some((from, to)) = m.compare_ends() else {
        ui.add_space(8.0);
        let note = if m.span == Span::FromRoot { t.tr("details.multi.span_from_root") } else { t.tr("details.multi.span_gaps") };
        ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
        return;
    };
    ui.separator();
    heading(ui, t, m, from, to, msgs);
    files(ui, t, st, m, from, to, msgs);
}

/// "Changes from X to Y" (with the count for a line of commits) and Swap direction.
fn heading(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, from: git2::Oid, to: git2::Oid, msgs: &mut Vec<Msg>) {
    let (f, o) = (short(from), short(to));
    let text = match m.span {
        Span::Range { .. } => t.trf("details.multi.changes_combined", &[("from", &f), ("to", &o), ("count", &m.len())]),
        _ => t.trf("details.multi.changes", &[("from", &f), ("to", &o)]),
    };
    // The button goes to its own line when the heading leaves no room for it.
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(text).strong().color(color::text()));
        let tip = t.trf("details.multi.swap.tip", &[("from", &o), ("to", &f)]);
        if ui.small_button(t.tr("details.multi.swap.button")).on_hover_text(tip).clicked() {
            msgs.push(Msg::Selection(SelectionMsg::SwapCompare));
        }
    });
}

/// The files that differ from `from` to `to`, each opening its diff between them.
fn files(ui: &mut egui::Ui, t: Lang, st: &AppState, m: &MultiSelection, from: git2::Oid, to: git2::Oid, msgs: &mut Vec<Msg>) {
    match &m.changes {
        None => {
            ui.label(RichText::new(t.tr("common.loading")).color(color::weak()));
        }
        Some(Err(e)) => {
            ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
        }
        Some(Ok(c)) if c.files.is_empty() => {
            ui.label(RichText::new(t.tr("details.multi.same_files")).color(color::weak()));
        }
        Some(Ok(c)) => {
            let open = st.diff.as_ref().map(|(target, _)| target);
            let lfs = TrackContext::of(st);
            let diff_blocked = crate::state::tool_compare::diff_blocked(&st.settings.external_tools);
            let row_h = widgets::LIST_ROW_H;
            egui::ScrollArea::vertical().id_salt("multi-files").auto_shrink([false, false]).show_rows(
                ui,
                row_h,
                c.files.len(),
                |ui, range| {
                    let index = |t: &DiffTarget| c.files.iter().position(|f| *t == DiffTarget::Between(from, to, f.path.clone()));
                    reveal(ui, "multi-files", st.view.file_reveal.as_ref(), index, &range, row_h);
                    for f in &c.files[range] {
                        let target = DiffTarget::Between(from, to, f.path.clone());
                        let src = RowSource::Between { from, to };
                        let in_workdir = c.in_workdir(&f.path);
                        let actions = RowActions { src, history_at: m.oids[0], in_workdir, lfs, diff_blocked, patch_blocked: None };
                        changed_file_row(ui, t, f, target, open, actions, msgs);
                    }
                },
            );
        }
    }
}
