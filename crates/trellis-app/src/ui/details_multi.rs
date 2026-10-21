//! Right panel for several selected commits: the list, actions on all of them, and for a pair or
//! one unbroken line of commits (from the empty tree when it starts at the first commit), the
//! files that differ across them (either way round) and their file actions; otherwise why there
//! is no combined diff, and each commit's own changes stacked (`details_stacked.rs`).

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
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};
use trellis_core::i18n::Lang;
use trellis_core::repo::{self, DiffTarget};

pub(super) fn panel(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(snap), Selection::Commits(m)) = (&st.snap, &st.selection) else { return };
    let t = st.settings.lang();
    ui.label(
        RichText::new(t.trn("details.multi.selected", m.len(), &[])).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text()),
    );
    gap_note(ui, t, m, snap);
    ui.horizontal_wrapped(|ui| {
        cherry_pick_button(ui, t, t.trn("details.multi.cherry_pick.button", m.len(), &[]), m, false, msgs);
        revert_button(ui, t, t.trn("details.multi.revert.button", m.len(), &[]), m, false, msgs);
        patch_button(ui, t, t.trn("details.multi.patch.button", m.len(), &[]), m, false, msgs);
        if ui.button(t.tr("details.multi.copy_shas.button")).clicked() {
            msgs.push(copy_shas(m));
        }
        ui.label(RichText::new(t.tr("details.multi.esc_hint")).size(type_scale::SMALL).color(color::weak()));
    });
    ui.add_space(6.0);

    let list_h = if m.compare_ends().is_some() || m.stack_oids().is_some() { 160.0 } else { ui.available_height() };
    // Only the rows in view are drawn: a range selection can span the whole history.
    let rows: Vec<(git2::Oid, usize)> = m.oids.iter().filter_map(|&o| Some((o, *snap.row_of.get(&o)?))).collect();
    let row_h = widgets::LIST_ROW_H;
    egui::ScrollArea::vertical().id_salt("multi-commits").max_height(list_h).auto_shrink([false, true]).show_rows(
        ui,
        row_h,
        rows.len(),
        |ui, range| {
            for &(oid, row) in &rows[range] {
                let summary = &snap.commits[row].summary;
                let r = widgets::file_row(ui, false, summary, |ui| {
                    ui.label(RichText::new(short(oid)).monospace().color(color::weak()));
                    widgets::row_text(ui, RichText::new(summary), false);
                })
                .response;
                if r.clicked() {
                    msgs.push(Msg::Repo(RepoMsg::SelectCommit(oid)));
                }
            }
        },
    );

    ui.separator();
    let Some((from, to)) = m.compare_ends() else {
        super::details_stacked::show(ui, t, st, m, snap, msgs);
        return;
    };
    heading(ui, t, m, from, to, msgs);
    files(ui, t, st, m, from, to, msgs);
}

/// Why there is no combined diff, under the title: the commit whose parent isn't selected, the
/// branches the selection splits into, or the shallow clone's boundary.
fn gap_note(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &trellis_core::repo::Snapshot) {
    let note = match (m.span, m.gap()) {
        (Span::Gaps { .. }, Some(c)) => {
            let merge = snap.row_of.get(&c).is_some_and(|&r| snap.commits[r].parents.len() > 1);
            let sha = [("sha", &short(c) as &dyn std::fmt::Display)];
            if merge { t.trf("details.multi.gap_merge", &sha) } else { t.trf("details.multi.gap", &sha) }
        }
        (Span::Gaps { .. }, None) => t.tr("details.multi.gap_branches").to_owned(),
        (Span::ShallowEnd, _) => t.tr("details.multi.span_shallow").to_owned(),
        (Span::Pair { .. } | Span::Range { .. }, _) => return,
    };
    ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
}

/// "Changes from X to Y" (with the count for a line of commits; a line from the first commit is
/// compared with an empty repository) and Swap direction, whose tip names the other way.
fn heading(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, from: git2::Oid, to: git2::Oid, msgs: &mut Vec<Msg>) {
    let (f, o) = (short(from), short(to));
    let count = m.len();
    let (text, tip) = match m.span {
        Span::Range { .. } if repo::is_empty_tree(from) => (
            t.trf("details.multi.changes_from_root", &[("to", &o), ("count", &count)]),
            t.trf("details.multi.swap.tip_to_root", &[("from", &o)]),
        ),
        Span::Range { .. } if repo::is_empty_tree(to) => (
            t.trf("details.multi.changes_to_root", &[("from", &f), ("count", &count)]),
            t.trf("details.multi.swap.tip_from_root", &[("to", &f)]),
        ),
        Span::Range { .. } => (
            t.trf("details.multi.changes_combined", &[("from", &f), ("to", &o), ("count", &count)]),
            t.trf("details.multi.swap.tip", &[("from", &o), ("to", &f)]),
        ),
        _ => (t.trf("details.multi.changes", &[("from", &f), ("to", &o)]), t.trf("details.multi.swap.tip", &[("from", &o), ("to", &f)])),
    };
    // The button goes to its own line when the heading leaves no room for it.
    ui.horizontal_wrapped(|ui| {
        ui.label(type_scale::section_text(ui, text));
        if ui.small_button(t.tr("details.multi.swap.button")).on_hover_text(tip).clicked() {
            msgs.push(Msg::Selection(SelectionMsg::SwapCompare));
        }
    });
}

/// The files that differ from `from` to `to`, each opening its diff between them.
fn files(ui: &mut egui::Ui, t: Lang, st: &AppState, m: &MultiSelection, from: git2::Oid, to: git2::Oid, msgs: &mut Vec<Msg>) {
    match &m.changes {
        None => super::pane_status::loading_line(ui, t.tr("common.loading")),
        Some(Err(e)) => {
            let retry = Some(Msg::Selection(SelectionMsg::RetryCompare));
            super::pane_status::failed_line(
                ui,
                t,
                super::pane_status::Failure { title: t.tr("details.multi.failed.title"), error: e, retry },
                msgs,
            );
        }
        Some(Ok(c)) if c.files.is_empty() => {
            ui.label(RichText::new(t.tr("details.multi.same_files")).color(color::weak()));
        }
        Some(Ok(c)) => {
            let open = st.diff.as_ref().map(|(target, _)| target);
            let lfs = TrackContext::of(st);
            let tools = crate::state::tool_compare::ToolSetup::of(st.settings.lang(), &st.settings.external_tools);
            let commands = crate::state::custom_commands::MenuCommands::for_state(st);
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
                        let actions = RowActions {
                            src,
                            history_at: m.oids[0],
                            in_workdir,
                            lfs,
                            tools,
                            patch_blocked: None,
                            lines: None,
                            commands: &commands,
                        };
                        changed_file_row(ui, t, f, target, open, actions, msgs);
                    }
                },
            );
        }
    }
}
