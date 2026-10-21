//! The stacked list of a multi-selection with no combined diff (not one line of history, or one
//! reaching a shallow clone's boundary): each selected commit's own changed files, newest first,
//! under a header naming the commit. Each file opens that commit's diff of it.

use super::common::short;
use super::details::file_reveal::reveal;
use super::details::{RowActions, changed_file_row};
use super::file_menu::RowSource;
use super::theme::color;
use super::widgets;
use crate::msg::{Msg, SelectionMsg};
use crate::state::AppState;
use crate::state::lfs::TrackContext;
use crate::state::selection::{CommitBlock, MultiSelection, STACK_LIMIT};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText, UiBuilder};
use trellis_core::i18n::Lang;
use trellis_core::repo::{DiffTarget, Snapshot, StackedCommit};

/// One row of the flattened list: a commit's header, one of its files, or its "No file changes".
#[derive(Clone, Copy)]
enum Row {
    Header(usize),
    File(usize, usize),
    Empty,
}

pub(super) fn show(ui: &mut egui::Ui, t: Lang, st: &AppState, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    ui.label(type_scale::section_text(ui, t.tr("details.multi.stacked.title")));
    if m.len() > STACK_LIMIT {
        let text = t.trf("details.multi.stacked.limited", &[("shown", &STACK_LIMIT), ("count", &m.len())]);
        ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
    }
    match &m.stacked {
        None => super::pane_status::loading_line(ui, t.tr("common.loading")),
        Some(Err(e)) => {
            let retry = Some(Msg::Selection(SelectionMsg::RetryCompare));
            let failure = super::pane_status::Failure { title: t.tr("details.multi.stacked.failed.title"), error: e, retry };
            super::pane_status::failed_line(ui, t, failure, msgs);
        }
        Some(Ok(stack)) => sections(ui, t, st, stack, snap, msgs),
    }
}

fn rows_of(stack: &[StackedCommit]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (i, c) in stack.iter().enumerate() {
        rows.push(Row::Header(i));
        if c.files.is_empty() {
            rows.push(Row::Empty);
        }
        rows.extend((0..c.files.len()).map(|j| Row::File(i, j)));
    }
    rows
}

/// Every section in one list, headers and files sharing one row height.
fn sections(ui: &mut egui::Ui, t: Lang, st: &AppState, stack: &[StackedCommit], snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let rows = rows_of(stack);
    let open = st.diff.as_ref().map(|(target, _)| target);
    let lfs = TrackContext::of(st);
    let tools = crate::state::tool_compare::ToolSetup::of(t, &st.settings.external_tools);
    let row_h = widgets::LIST_ROW_H;
    let parents = |c: &StackedCommit| snap.row_of.get(&c.oid).map_or(1, |&r| snap.commits[r].parents.len());
    egui::ScrollArea::vertical().id_salt("multi-stacked").auto_shrink([false, false]).show_rows(ui, row_h, rows.len(), |ui, range| {
        let index = |want: &DiffTarget| {
            rows.iter().position(|r| match *r {
                Row::File(i, j) => *want == DiffTarget::Commit(stack[i].oid, stack[i].files[j].path.clone()),
                _ => false,
            })
        };
        reveal(ui, "multi-stacked", st.view.file_reveal.as_ref(), index, &range, row_h);
        for &row in &rows[range] {
            match row {
                Row::Header(i) => header(ui, t, &stack[i], snap, parents(&stack[i]) > 1),
                Row::Empty => {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
                    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
                    child.label(RichText::new(t.tr("details.multi.stacked.no_files")).color(color::weak()));
                }
                Row::File(i, j) => {
                    let (c, f) = (&stack[i], &stack[i].files[j]);
                    let merge = parents(c) > 1;
                    let actions = RowActions {
                        src: RowSource::Commit(c.oid),
                        history_at: c.oid,
                        in_workdir: c.in_workdir(&f.path),
                        lfs,
                        tools,
                        patch_blocked: merge.then(|| CommitBlock::PatchMerge.text(t)),
                        lines: None,
                    };
                    changed_file_row(ui, t, f, DiffTarget::Commit(c.oid, f.path.clone()), open, actions, msgs);
                }
            }
        }
    });
}

/// A section's header: the commit's short SHA and summary, cut to leave room for its file count
/// (and, for a merge, that its changes are against its first parent).
fn header(ui: &mut egui::Ui, t: Lang, c: &StackedCommit, snap: &Snapshot, merge: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), widgets::LIST_ROW_H), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let mut row = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    row.label(RichText::new(t.trn("details.multi.stacked.files", c.files.len(), &[])).size(type_scale::SMALL).color(color::weak()));
    if merge {
        row.label(RichText::new(t.tr("details.multi.stacked.merge_note")).size(type_scale::SMALL).color(color::weak()));
    }
    row.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.label(RichText::new(short(c.oid)).monospace().color(color::weak()));
        let summary = snap.row_of.get(&c.oid).map_or("", |&r| snap.commits[r].summary.as_str());
        widgets::row_text(ui, RichText::new(summary).color(color::strong()), false);
    });
}
