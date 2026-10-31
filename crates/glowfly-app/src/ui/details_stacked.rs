//! The stacked list of a multi-selection with no combined diff (not one line of history, or one
//! reaching a shallow clone's boundary): each selected commit's own changed files, newest first,
//! under a header naming the commit. Each file opens that commit's diff of it. In Tree mode each
//! section's files are grouped in folders, each section keeping its own folds. The list takes the
//! keyboard focus as a whole (`file_list_keys.rs`), its keys stepping across the sections.

use super::common::short;
use super::details::file_reveal::reveal;
use super::details::{RowActions, changed_file_row_at};
use super::file_menu::RowSource;
use super::file_menu::folder_menu::{self, FolderStage};
use super::list_row::file_tree::{ListRows, Shown, folder_row, mode_toggle};
use super::theme::color;
use super::widgets;
use crate::msg::{Msg, SelectionMsg};
use crate::state::AppState;
use crate::state::file_folds::FileListId;
use crate::state::lfs::TrackContext;
use crate::state::list_keys::FileList;
use crate::state::selection::{CommitBlock, MultiSelection, STACK_LIMIT};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText, UiBuilder};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, Snapshot, StackedCommit};

/// One row of the flattened list: a commit's header, one of its files (at its depth in the
/// section's tree, None in List mode), one of its folder rows (by row of the section's tree),
/// or its "No file changes".
#[derive(Clone, Copy)]
enum Row {
    Header(usize),
    File(usize, usize, Option<usize>),
    Folder(usize, usize),
    Empty,
}

pub(super) fn show(ui: &mut egui::Ui, t: Lang, st: &AppState, m: &MultiSelection, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    super::details_wip::header::frame(ui, |ui| {
        mode_toggle(ui, t, st.settings.file_list_mode, msgs);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(type_scale::section_text(ui, t.tr("details.multi.stacked.title")));
        });
    });
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

/// The flattened list, `sections` holding each commit's file rows.
fn rows_of(stack: &[StackedCommit], sections: &[ListRows]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (i, (c, section)) in stack.iter().zip(sections).enumerate() {
        rows.push(Row::Header(i));
        if c.files.is_empty() {
            rows.push(Row::Empty);
        }
        rows.extend((0..section.len()).map(|k| match section.get(k) {
            Shown::File { index, depth } => Row::File(i, index, depth),
            Shown::Folder(_) => Row::Folder(i, k),
        }));
    }
    rows
}

/// Every section in one list, headers and files sharing one row height.
fn sections(ui: &mut egui::Ui, t: Lang, st: &AppState, stack: &[StackedCommit], snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let mode = st.settings.file_list_mode;
    let sections: Vec<ListRows> = stack
        .iter()
        .map(|c| {
            let paths = || c.files.iter().map(|f| f.path.as_str()).collect();
            ListRows::of(ui, mode, c.files.len(), paths, st.session.tab_view.file_folds.closed(FileListId::Stacked(c.oid)))
        })
        .collect();
    let rows = rows_of(stack, &sections);
    let open = crate::state::wip_lists::cursor(st);
    let lfs = TrackContext::of(st);
    let tools = crate::state::tool_compare::ToolSetup::of(t, &st.settings.external_tools);
    let commands = crate::state::custom_commands::MenuCommands::for_state(st);
    let row_h = widgets::LIST_ROW_H;
    let parents = |c: &StackedCommit| snap.row_of.get(&c.oid).map_or(1, |&r| snap.commits[r].parents.len());
    let index = |want: &DiffTarget| {
        rows.iter().position(|r| match *r {
            Row::File(i, j, _) => *want == DiffTarget::Commit(stack[i].oid, stack[i].files[j].path.clone()),
            _ => false,
        })
    };
    let current = open.and_then(index);
    let focus =
        super::file_list_keys::target(ui, FileList::Stacked, rows.len(), ui.available_height(), t.tr("details.multi.stacked.title"));
    let area = egui::ScrollArea::vertical().id_salt("multi-stacked").auto_shrink([false, false]);
    let shown = area.show_rows(ui, row_h, rows.len(), |ui, range| {
        reveal(ui, "multi-stacked", st.view.file_reveal.as_ref(), index, &range, row_h);
        for at in range {
            match rows[at] {
                Row::Header(i) => header(ui, t, &stack[i], snap, parents(&stack[i]) > 1),
                Row::Empty => {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
                    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
                    child.label(RichText::new(t.tr("details.multi.stacked.no_files")).color(color::weak()));
                }
                Row::Folder(i, k) => {
                    let Shown::Folder(folder) = sections[i].get(k) else { continue };
                    let seen = Some(stack[i].oid);
                    let menu =
                        |ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str| folder_menu::items(ui, t, msgs, path, seen, FolderStage::None);
                    let r = folder_row(ui, folder, true, msgs, FileListId::Stacked(stack[i].oid), menu);
                    focus.row(ui, &r, false);
                }
                Row::File(i, j, depth) => {
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
                        commands: &commands,
                    };
                    let r = changed_file_row_at(ui, t, f, (DiffTarget::Commit(c.oid, f.path.clone()), open), actions, depth, msgs);
                    focus.row(ui, &r, current == Some(at));
                }
            }
        }
    });
    focus.rest(ui, &shown, current.is_some());
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
