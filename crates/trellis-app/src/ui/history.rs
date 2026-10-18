//! The file and folder history view: the commits that changed a file (newest first, following
//! renames) or anything under a folder on the left, and the selected commit on the right with
//! its diff of the file or, in the File view, the whole file as of it (`history_file.rs`). A
//! folder entry lists its changed files (`history_folder.rs`), one of which is shown. Each
//! commit can restore its version of a file; Open and Show act on the file as it is today.

use super::UiCache;
use super::common::{menu_item, menu_item_if, paint_cut_line, short, status_color};
use super::file_buttons::{self, CommitFile, Labels};
use super::file_menu;
use super::theme::color;
use crate::msg::BlameMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::history::{FileHistoryView, HistoryKind, HistoryPane};
use crate::state::tool_compare::{OPEN_IN_DIFF_TOOL, diff_blocked};
use crate::tools::ToolsMsg;
use eframe::egui::{self, Align, Align2, CursorIcon, FontId, Key, Layout, RichText, Sense, Ui, pos2, vec2};
use trellis_core::repo::{DiffTarget, HistoryEntry};

const ROW_H: f32 = 44.0;
/// The row menu's item that blames the file as of the entry's commit.
pub(super) const BLAME_AT: &str = "Blame at this commit";
pub(super) const NOT_ON_GRAPH: &str = "Not visible on the graph: its branch is hidden or it is older than the commit limit";

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.history_scroll_to.take();
    let Some(view) = &st.history else { return };
    ui.add_space(6.0);
    header(ui, view, msgs);
    ui.separator();
    egui::SidePanel::left("file-history-list")
        .resizable(true)
        .default_width(380.0)
        .width_range(260.0..=600.0)
        .frame(egui::Frame::new().fill(color::panel()))
        .show_inside(ui, |ui| list(ui, st, scroll_to, msgs));
    egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg())).show_inside(ui, |ui| selected(ui, st, cache, msgs));
}

/// "File history" or "Folder history", the path ("now <name>" when the file was renamed after
/// the commit it was opened at), "as of <sha>" when the walk didn't start at HEAD, then the
/// Diff / File toggle, Blame and Close.
fn header(ui: &mut Ui, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let (title, path) = match view.kind {
            HistoryKind::File => ("File history", view.path.clone()),
            HistoryKind::Folder => ("Folder history", format!("{}/", view.path)),
        };
        ui.label(RichText::new(title).color(color::weak()));
        ui.label(RichText::new(path).strong().size(15.0).color(color::text()));
        if let (HistoryKind::File, Some(now)) = (view.kind, &view.head_path) {
            ui.label(RichText::new(format!("now {now}")).color(color::weak()))
                .on_hover_text("Renamed after this commit; the history starts at HEAD under its current name");
        }
        if let Some(at) = view.as_of {
            ui.label(RichText::new(format!("as of {}", short(at))).color(color::weak()))
                .on_hover_text("The history as seen from this commit (it is not part of HEAD's history of this file)");
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::History(HistoryMsg::CloseFileHistory));
            }
            let shown = view.shown();
            let blocked = match shown {
                None => Some("Select a commit first"),
                Some((_, _, 'D')) => Some("This commit deletes the file"),
                Some(_) => None,
            };
            let r = ui.add_enabled(blocked.is_none(), egui::Button::new(file_menu::BLAME));
            let r = match blocked {
                Some(why) => r.on_disabled_hover_text(why),
                None => r.on_hover_text("Blame the file as of the selected commit"),
            };
            if r.clicked()
                && let Some((oid, path, _)) = shown
            {
                msgs.push(Msg::Blame(BlameMsg::Open { path: path.to_owned(), at: Some(oid) }));
            }
            pane_toggle(ui, view, msgs);
        });
    });
}

/// The Diff / File segmented toggle (laid out right to left: File, then Diff to its left).
fn pane_toggle(ui: &mut Ui, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    ui.add_enabled_ui(view.shown().is_some(), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let buttons = [
                (HistoryPane::File, "File", "The whole file as of this commit"),
                (HistoryPane::Diff, "Diff", "This commit's changes to the file"),
            ];
            for (pane, label, tip) in buttons {
                let r = ui.selectable_label(view.pane == pane, label).on_hover_text(tip).on_disabled_hover_text("Select a commit first");
                if r.clicked() && view.pane != pane {
                    msgs.push(Msg::History(HistoryMsg::SetHistoryPane(pane)));
                }
            }
        });
    });
}

fn list(ui: &mut Ui, st: &AppState, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.history else { return };
    keyboard(ui, view, msgs);
    let in_wd = view.in_workdir(st.snap.as_ref());
    let diff_blocked = diff_blocked(&st.settings.external_tools);
    egui::ScrollArea::vertical().id_salt("file-history").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, e) in view.entries.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
            if scroll_to == Some(i) {
                resp.scroll_to_me(Some(Align::Center));
            }
            let date_rect = ui.is_rect_visible(rect).then(|| {
                let selected = view.selected == Some(i);
                if selected {
                    ui.painter().rect_filled(rect, 0.0, color::selected());
                    ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, vec2(3.0, ROW_H)), 0.0, color::accent());
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, color::hover());
                }
                row(ui, st, rect, e, view.kind)
            });
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let resp = match (date_rect, ui.ctx().pointer_hover_pos()) {
                (Some(r), Some(p)) if resp.hovered() && r.contains(p) => {
                    resp.on_hover_text_at_pointer(st.settings.date_tooltip(e.time, st.view.now, st.view.utc_offset))
                }
                _ => resp,
            };
            if resp.clicked() {
                msgs.push(Msg::History(HistoryMsg::SelectHistoryEntry(i)));
            }
            resp.context_menu(|ui| match view.kind {
                HistoryKind::File => entry_menu(ui, msgs, e, view.today_path(), in_wd, diff_blocked),
                HistoryKind::Folder => commit_items(ui, msgs, e),
            });
            ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
        }
        ui.add_space(8.0);
        footer(ui, st, view, msgs);
    });
}

/// View on graph and Copy commit SHA: the whole menu of a folder history row.
fn commit_items(ui: &mut Ui, msgs: &mut Vec<Msg>, e: &HistoryEntry) {
    menu_item(msgs, ui, "View on graph", Msg::History(HistoryMsg::ShowInGraph(e.oid)));
    menu_item(msgs, ui, "Copy commit SHA", Msg::Copy(e.oid.to_string()));
}

/// A history row's menu: View on graph, Copy commit SHA, Blame at this commit (not for the commit
/// that deletes the file), then Restore for the entry's version, Revert changes to this file
/// (the entry's change, disabled unless `in_workdir` or the entry deleted the file) and Open / Open in external editor / Show for `open_path` (the file as it is named today),
/// disabled unless `in_workdir`, with Open in external diff tool for the entry's change
/// (disabled with `diff_blocked`).
pub(super) fn entry_menu(
    ui: &mut Ui,
    msgs: &mut Vec<Msg>,
    e: &HistoryEntry,
    open_path: &str,
    in_workdir: bool,
    diff_blocked: Option<&str>,
) {
    commit_items(ui, msgs, e);
    let deleted = (e.status == 'D').then_some("This commit deletes the file");
    menu_item_if(msgs, ui, BLAME_AT, deleted, Msg::Blame(BlameMsg::Open { path: e.path.clone(), at: Some(e.oid) }));
    ui.separator();
    file_menu::restore_item(ui, msgs, e.oid, &e.path, e.status);
    // update/revert.rs maps an older name to `open_path`, which `in_workdir` is about.
    let revert_blocked = (!in_workdir && e.status != 'D').then_some(file_menu::NOT_IN_WORKDIR);
    file_menu::revert_file_item(ui, msgs, DiffTarget::Commit(e.oid, e.path.clone()), revert_blocked);
    let tool = file_menu::diff_item(DiffTarget::Commit(e.oid, e.path.clone()), diff_blocked);
    file_menu::open_items(ui, msgs, open_path, (!in_workdir).then_some(file_menu::NOT_IN_WORKDIR), Some(tool));
}

/// Line 1: short SHA and summary. Line 2: status, author (and the old path of a rename, or a
/// folder entry's file count), with the date on the right. Returns where the date was painted.
fn row(ui: &Ui, st: &AppState, rect: egui::Rect, e: &HistoryEntry, kind: HistoryKind) -> egui::Rect {
    let mono = FontId::monospace(12.0);
    let body = FontId::proportional(13.0);
    let small = FontId::proportional(12.0);
    let (left, right) = (rect.left() + 12.0, rect.right() - 10.0);
    let y1 = rect.top() + 13.0;
    let y2 = rect.top() + 31.0;

    let sha_w = paint_cut_line(ui, pos2(left, y1), &short(e.oid), mono.clone(), color::weak(), 80.0);
    paint_cut_line(ui, pos2(left + sha_w + 8.0, y1), &e.summary, body, color::text(), right - left - sha_w - 8.0);

    let date = st.settings.format_day(e.time, st.view.now, st.view.utc_offset);
    let date_rect = ui.painter().text(pos2(right, y2), Align2::RIGHT_CENTER, date, small.clone(), color::weak());
    let date_w = date_rect.width();
    let status_w = paint_cut_line(ui, pos2(left, y2), &e.status.to_string(), mono, status_color(e.status), 20.0);
    let mut x = left + status_w.max(8.0) + 8.0;
    let limit = right - date_w - 12.0;
    let author = if e.author.is_empty() { "Unknown" } else { e.author.as_str() };
    x += paint_cut_line(ui, pos2(x, y2), author, small.clone(), color::text(), limit - x);
    let extra = match kind {
        HistoryKind::Folder if e.files.len() == 1 => Some("1 file".to_owned()),
        HistoryKind::Folder => Some(format!("{} files", e.files.len())),
        HistoryKind::File => e.old_path.as_ref().map(|old| format!("renamed from {old}")),
    };
    if let Some(extra) = extra {
        x += 10.0;
        paint_cut_line(ui, pos2(x, y2), &extra, small, color::weak(), limit - x);
    }
    date_rect
}

fn footer(ui: &mut Ui, st: &AppState, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 10, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        if view.loading {
            ui.label(RichText::new("Loading…").color(color::weak()));
        } else if let Some(e) = &view.error {
            ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
        } else if view.complete() {
            if view.entries.is_empty() {
                let none = match view.kind {
                    HistoryKind::File => "No commits change this file",
                    HistoryKind::Folder => "No commits change this folder",
                };
                ui.label(RichText::new(none).color(color::weak()));
            } else {
                ui.label(RichText::new("Reached the first commit").color(color::faint()));
            }
        } else {
            // Stopped at the commit limit.
            if let Some((oid, time)) = view.searched_until {
                let date = st.settings.format_date_word(time, st.view.utc_offset);
                ui.label(RichText::new(format!("Searched back to {date} ({})", short(oid))).color(color::weak()));
            }
            if ui.button("Search further").clicked() {
                msgs.push(Msg::History(HistoryMsg::MoreFileHistory));
            }
        }
    });
    ui.add_space(8.0);
}

/// Up and Down move the selection while no text field has focus.
fn keyboard(ui: &Ui, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    if view.entries.is_empty() || ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }
    let (up, down) =
        ui.input(|i| (i.modifiers.is_none() && i.key_pressed(Key::ArrowUp), i.modifiers.is_none() && i.key_pressed(Key::ArrowDown)));
    let next = match (view.selected, up, down) {
        (Some(s), true, false) if s > 0 => s - 1,
        (Some(s), false, true) if s + 1 < view.entries.len() => s + 1,
        (None, _, true) | (None, true, _) => 0,
        _ => return,
    };
    msgs.push(Msg::History(HistoryMsg::SelectHistoryEntry(next)));
}

/// The selected commit: summary, author and date, links and (in a file history) file buttons
/// (Restore this version, Open, Show, Open in external diff tool), or (in a folder history) its
/// changed files; then its diff of the file, or the whole file in the File view.
fn selected(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.history else { return };
    let (kind, pane, target) = (view.kind, view.pane, view.target());
    let open_path = view.today_path().to_owned();
    let in_wd = view.in_workdir(st.snap.as_ref());
    let Some(e) = view.selected_entry().cloned() else {
        if !view.loading && !view.entries.is_empty() {
            ui.add_space(10.0);
            ui.label(RichText::new("Select a commit to see its changes to the file.").color(color::weak()));
        }
        return;
    };
    let visible = st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&e.oid));
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(&e.summary).strong().size(14.0).color(color::text())).truncate());
        let author = if e.author.is_empty() { "Unknown" } else { e.author.as_str() };
        let when = st.settings.format_verbose(e.time, st.view.utc_offset);
        ui.label(RichText::new(format!("{author}, {when}")).color(color::weak()));
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("commit {}", short(e.oid))).monospace().color(color::weak()));
            let link = ui.add_enabled(visible, egui::Link::new("View on graph")).on_disabled_hover_text(NOT_ON_GRAPH);
            if link.clicked() {
                msgs.push(Msg::History(HistoryMsg::ShowInGraph(e.oid)));
            }
            if ui.small_button("Copy SHA").clicked() {
                msgs.push(Msg::Copy(e.oid.to_string()));
            }
        });
        ui.add_space(4.0);
        match kind {
            HistoryKind::File => {
                let file = CommitFile { oid: e.oid, path: &e.path, status: e.status, open_path: &open_path, in_workdir: in_wd };
                let blocked = diff_blocked(&st.settings.external_tools);
                ui.horizontal_wrapped(|ui| {
                    file_buttons::show(ui, msgs, &file, Labels::Full);
                    let r = ui.add_enabled(blocked.is_none(), egui::Button::new(OPEN_IN_DIFF_TOOL));
                    if r.on_disabled_hover_text(blocked.unwrap_or_default()).clicked() {
                        msgs.push(Msg::Tools(ToolsMsg::OpenDiff(DiffTarget::Commit(e.oid, e.path.clone()))));
                    }
                });
            }
            HistoryKind::Folder => super::history_folder::files(ui, st, &e, msgs),
        }
    });
    ui.separator();
    let Some(target) = target else { return };
    match pane {
        HistoryPane::File => super::history_file::show(ui, st),
        HistoryPane::Diff if st.diff.as_ref().is_some_and(|(t, _)| *t == target) => super::diff::body(ui, st, &mut cache.diff_rows, msgs),
        HistoryPane::Diff => {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new("Loading…").color(color::weak()));
            });
        }
    }
}
