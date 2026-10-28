//! The file and folder history view: the commits that changed a file (newest first, following
//! renames) or anything under a folder on the left, and the selected commit on the right with
//! its diff of the file or, in the File view, the whole file as of it (`history_file.rs`); the
//! header's Diff | File | Blame switch picks between those and the blame as of it. A
//! folder entry lists its changed files (`history_folder.rs`), one of which is shown. Each
//! commit can restore its version of a file; Open and Show act on the file as it is today.

use super::UiCache;
use super::common::{menu_item, menu_item_if, short, status_color, with_code};
use super::empty_state::{EmptyIcon, empty_state};
use super::file_buttons::{self, CommitFile};
use super::file_menu;
use super::file_view_switch::{self, FileShow};
use super::theme::color;
use super::widgets::{self, paint_cut_text};
use crate::keymap::Keymap;
use crate::msg::BlameMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::history::{FileHistoryView, HistoryKind, HistoryPane};
use crate::state::tool_compare::{OPEN_IN_DIFF_TOOL, ToolSetup};
use crate::tools::ToolsMsg;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align, Align2, CursorIcon, Key, Layout, RichText, Sense, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, HistoryEntry};

const ROW_H: f32 = 44.0;
/// The row menu's item that blames the file as of the entry's commit.
pub(super) const BLAME_AT: &str = "Blame at this commit";
pub(super) const NO_SELECTION_TITLE: &str = "No commit selected";
const NO_SELECTION_HINT: &str = "Select a commit to see its changes to the file.";
const LOADING_CHANGES: &str = "Loading the changes…";
pub(super) const NOT_ON_GRAPH: &str = "Not visible on the graph: its branch is hidden or it is older than the commit limit";

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.history_scroll_to.take();
    let file_scroll = st.view.history_file_scroll.take();
    let Some(view) = &st.session.history.view else { return };
    ui.add_space(6.0);
    header(ui, st.settings.lang(), &st.keymap, view, msgs);
    ui.separator();
    let room = ui.available_width();
    super::panel_size::inner_side(egui::SidePanel::left("file-history-list"), crate::state::panel_layout::HISTORY_LIST, room)
        .frame(egui::Frame::new().fill(color::panel()))
        .show_inside(ui, |ui| list(ui, st, scroll_to, msgs));
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(color::bg()))
        .show_inside(ui, |ui| selected(ui, st, cache, file_scroll, msgs));
}

/// "File history" or "Folder history", the path ("now <name>" when the file was renamed after
/// the commit it was opened at), "as of <sha>" when the walk didn't start at HEAD, then the
/// Diff | File | Blame switch and Close.
fn header(ui: &mut Ui, t: Lang, keys: &Keymap, view: &FileHistoryView, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let (title, path) = match view.kind {
            HistoryKind::File => ("File history", view.path.clone()),
            HistoryKind::Folder => ("Folder history", format!("{}/", view.path)),
        };
        ui.label(RichText::new(title).color(color::weak()));
        ui.label(RichText::new(path).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text()));
        if let (HistoryKind::File, Some(now)) = (view.kind, &view.head_path) {
            ui.label(RichText::new(format!("now {now}")).color(color::weak()))
                .on_hover_text("Renamed after this commit; the history starts at HEAD under its current name");
        }
        if let Some(at) = view.as_of {
            ui.label(RichText::new(format!("as of {}", short(at))).color(color::weak()))
                .on_hover_text("The history as seen from this commit (it is not part of HEAD's history of this file)");
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::ghost::close_view(ui, keys, t, t.tr("history.header.close.name")).clicked() {
                msgs.push(Msg::History(HistoryMsg::CloseFileHistory));
            }
            ui.add_space(8.0);
            let shown = view.shown();
            let all_off = shown.is_none().then_some(file_view_switch::NO_COMMIT);
            let blame_off = matches!(shown, Some((_, _, 'D'))).then_some(file_view_switch::DELETES);
            match file_view_switch::show(ui, view.pane.into(), all_off, blame_off) {
                Some(FileShow::Diff) => msgs.push(Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::Diff))),
                Some(FileShow::File) => msgs.push(Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::File))),
                Some(FileShow::Blame) => {
                    if let Some((oid, path, _)) = shown {
                        msgs.push(Msg::Blame(BlameMsg::Open { path: path.to_owned(), at: Some(oid) }));
                    }
                }
                None => {}
            }
        });
    });
}

fn list(ui: &mut Ui, st: &AppState, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.session.history.view else { return };
    keyboard(ui, view, msgs);
    let in_wd = view.in_workdir(st.snap.as_ref());
    let tools = ToolSetup::of(st.settings.lang(), &st.settings.external_tools);
    if view.entries.is_empty() {
        return super::history_footer::empty_list(ui, st, view, msgs);
    }
    egui::ScrollArea::vertical().id_salt("file-history").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, e) in view.entries.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
            if scroll_to == Some(i) {
                resp.scroll_to_me(Some(Align::Center));
            }
            let date_rect = ui.is_rect_visible(rect).then(|| {
                let selected = view.selected == Some(i);
                super::widgets::row_fill::paint(ui.painter(), rect, selected, resp.hovered());
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
            resp.row_menu(|ui| match view.kind {
                HistoryKind::File => entry_menu(ui, st.settings.lang(), msgs, e, view.today_path(), in_wd, tools),
                HistoryKind::Folder => commit_items(ui, msgs, e),
            });
            ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
        }
        ui.add_space(8.0);
        super::history_footer::footer(ui, st, view, msgs);
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
/// disabled unless `in_workdir`, with Open in external diff tool for the entry's change (the
/// editor and diff tool entries say "Set up…" while `tools` has no program for them).
pub(super) fn entry_menu(ui: &mut Ui, t: Lang, msgs: &mut Vec<Msg>, e: &HistoryEntry, open_path: &str, in_workdir: bool, tools: ToolSetup) {
    commit_items(ui, msgs, e);
    let deleted = (e.status == 'D').then_some("This commit deletes the file");
    menu_item_if(msgs, ui, BLAME_AT, deleted, Msg::Blame(BlameMsg::Open { path: e.path.clone(), at: Some(e.oid) }));
    crate::ui::widgets::menu::separator(ui);
    file_menu::restore_item(ui, t, msgs, e.oid, &e.path, e.status);
    // update/revert.rs maps an older name to `open_path`, which `in_workdir` is about.
    let revert_blocked = (!in_workdir && e.status != 'D').then_some(file_menu::NOT_IN_WORKDIR);
    file_menu::revert_file_item(ui, t, msgs, DiffTarget::Commit(e.oid, e.path.clone()), revert_blocked);
    let tool = file_menu::diff_item(t, DiffTarget::Commit(e.oid, e.path.clone()), tools.diff);
    file_menu::open_items(ui, t, msgs, open_path, (!in_workdir).then_some(file_menu::NOT_IN_WORKDIR), tools.editor, Some(tool));
}

/// Line 1: short SHA and summary. Line 2: status, author (and the old path of a rename, or a
/// folder entry's file count), with the date on the right. Returns where the date was painted.
fn row(ui: &Ui, st: &AppState, rect: egui::Rect, e: &HistoryEntry, kind: HistoryKind) -> egui::Rect {
    let mono = type_scale::mono(type_scale::SMALL);
    let body = type_scale::font(type_scale::BODY);
    let small = type_scale::font(type_scale::SMALL);
    let (left, right) = (rect.left() + 12.0, rect.right() - 10.0);
    let y1 = rect.top() + 13.0;
    let y2 = rect.top() + 31.0;

    let p = ui.painter();
    let sha_w = paint_cut_text(p, pos2(left, y1), &short(e.oid), mono.clone(), color::weak(), 80.0).rect.width();
    paint_cut_text(p, pos2(left + sha_w + 8.0, y1), &e.summary, body, color::text(), right - left - sha_w - 8.0);

    let date = st.settings.format_day(e.time, st.view.now, st.view.utc_offset);
    let date_rect = ui.painter().text(pos2(right, y2), Align2::RIGHT_CENTER, date, small.clone(), color::weak());
    let date_w = date_rect.width();
    let status_w = paint_cut_text(p, pos2(left, y2), &e.status.to_string(), mono, status_color(e.status), 20.0).rect.width();
    let mut x = left + status_w.max(8.0) + 8.0;
    let limit = right - date_w - 12.0;
    let author = if e.author.is_empty() { "Unknown" } else { e.author.as_str() };
    x += paint_cut_text(p, pos2(x, y2), author, small.clone(), color::text(), limit - x).rect.width();
    let extra = match kind {
        HistoryKind::Folder => {
            let n = if e.files.len() == 1 { "1 file".to_owned() } else { format!("{} files", e.files.len()) };
            Some(match &e.old_path {
                Some(old) => format!("{n} · moved from {old}/"),
                None => n,
            })
        }
        HistoryKind::File => e.old_path.as_ref().map(|old| format!("renamed from {old}")),
    };
    if let Some(extra) = extra {
        x += 10.0;
        paint_cut_text(p, pos2(x, y2), &extra, small, color::weak(), limit - x);
    }
    date_rect
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

/// The selected commit: summary, author and date, links, (in a folder history) its changed
/// files, and file buttons for the file shown (Restore this version, Open, Show, Open in
/// external diff tool); then its diff of the file, or the whole file in the File view.
fn selected(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, file_scroll: Option<usize>, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.session.history.view else { return };
    let (kind, pane, target) = (view.kind, view.pane, view.target());
    let open_path = view.shown_today();
    let in_wd = view.shown_in_workdir(st.snap.as_ref());
    let shown = view.shown().map(|(oid, path, status)| (oid, path.to_owned(), status));
    let Some(e) = view.selected_entry().cloned() else {
        if !view.loading && !view.entries.is_empty() {
            empty_state(ui, EmptyIcon::File, NO_SELECTION_TITLE, NO_SELECTION_HINT, &[]);
        }
        return;
    };
    let visible = st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&e.oid));
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(&e.summary).font(type_scale::strong(ui, type_scale::BODY)).color(color::text())).truncate());
        let author = if e.author.is_empty() { "Unknown" } else { e.author.as_str() };
        let when = st.settings.format_verbose(e.time, st.view.utc_offset);
        ui.label(RichText::new(format!("{author}, {when}")).color(color::weak()));
        ui.horizontal(|ui| {
            ui.label(with_code("Commit {sha}", "{sha}", &short(e.oid), type_scale::SMALL, color::weak()));
            let link = ui.add_enabled(visible, egui::Link::new("View on graph")).on_disabled_hover_text(NOT_ON_GRAPH);
            if link.clicked() {
                msgs.push(Msg::History(HistoryMsg::ShowInGraph(e.oid)));
            }
            if ui.add(buttons::secondary("Copy SHA").small()).clicked() {
                msgs.push(Msg::Copy(e.oid.to_string()));
            }
        });
        ui.add_space(4.0);
        if kind == HistoryKind::Folder {
            super::history_folder::files(ui, st, &e, file_scroll, msgs);
        }
        if let (Some((oid, path, status)), Some(open_path)) = (&shown, &open_path) {
            if kind == HistoryKind::Folder {
                ui.add_space(4.0);
            }
            let file = CommitFile { oid: *oid, path, status: *status, open_path, in_workdir: in_wd };
            file_actions(ui, st, msgs, &file);
        }
    });
    ui.separator();
    let Some(target) = target else { return };
    match pane {
        HistoryPane::File => super::history_file::show(ui, st, cache, msgs),
        HistoryPane::Diff => match st.for_diff() {
            Some(p) => super::content_fetch::show(ui, st, p, msgs),
            None if st.session.diff.view.as_ref().is_some_and(|(t, _)| *t == target) => super::diff::body(ui, st, cache, msgs),
            None => super::pane_status::loading(ui, LOADING_CHANGES),
        },
    }
}

/// Restore, Open and Show for `f`, then Open in external diff tool for its change (always
/// enabled: with no tool set it says on hover that it opens Preferences > External Tools).
fn file_actions(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>, f: &CommitFile) {
    let setup = ToolSetup::of(st.settings.lang(), &st.settings.external_tools).diff;
    ui.horizontal_wrapped(|ui| {
        file_buttons::show(ui, msgs, f);
        let mut button = buttons::secondary(OPEN_IN_DIFF_TOOL);
        if let Some(note) = setup {
            button = button.tip(note);
        }
        if ui.add(button).clicked() {
            msgs.push(Msg::Tools(ToolsMsg::OpenDiff(DiffTarget::Commit(f.oid, f.path.to_owned()))));
        }
    });
}
