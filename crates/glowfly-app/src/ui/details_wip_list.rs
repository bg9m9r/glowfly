//! The Unstaged and Staged Files lists of the WIP panel (and of the operation steps), in List
//! or Tree mode; their headers are in `details_wip_header.rs`, their rows in `details_wip.rs`.

use super::{OpenFile, Reveal, RowContext, header, wip_file_row};
use crate::msg::Msg;
use crate::state::file_folds::FileListId;
use crate::state::file_tree::under;
use crate::ui::file_menu;
use crate::ui::file_menu::folder_menu::{self, FolderStage};
use crate::ui::list_row::file_tree::{ListRows, Shown, folder_row};
use crate::ui::theme::color;
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, RichText};
use glowfly_core::repo::{DiffTarget, FileEntry, Snapshot};

/// The Unstaged or Staged Files list of the working tree. Conflicted files have their own list
/// (`conflict_files::show`), so Unstaged Files leaves them out. It scrolls to the file a step
/// landed on. The list takes the keyboard focus as one Tab stop, or by a click on a row; while
/// it has it, Up/Down step between its files and its current row (or, with none, the list) is
/// ringed. In Tree mode its files are grouped under folder rows, which a click collapses.
pub(in crate::ui) fn file_list(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    staged: bool,
    (open, want): (&OpenFile, Reveal),
    max_h: f32,
    ctx: RowContext,
    msgs: &mut Vec<Msg>,
) {
    // The English titles stay the lists' scroll ids, whatever the language.
    let (id, all) = if staged { ("Staged files", &snap.staged) } else { ("Unstaged files", &snap.unstaged) };
    // Built only while files are conflicted: a working tree can have thousands of changed files.
    let kept: Option<Vec<&FileEntry>> =
        (!staged && all.iter().any(|f| f.status == '!')).then(|| all.iter().filter(|f| f.status != '!').collect());
    let count = kept.as_ref().map_or(all.len(), Vec::len);
    let t = ctx.lang;
    let title = if staged {
        t.trf("details.wip.staged.header", &[("count", &count)])
    } else {
        t.trf("details.wip.unstaged.header", &[("count", &count)])
    };
    let header = header::header(ui, title.clone(), staged, count, ctx, msgs);
    if staged {
        header.row_menu(|ui| file_menu::staged_header_items(ui, t, msgs, snap));
    }
    let entry = |i: usize| kept.as_ref().map_or(&all[i], |k| k[i]);
    let list = if staged { FileListId::Staged } else { FileListId::Unstaged };
    let rows = ListRows::of(ui, ctx.mode, count, || (0..count).map(|i| entry(i).path.as_str()).collect(), ctx.folds.closed(list));
    let row_h = crate::ui::widgets::LIST_ROW_H;
    // The list's area, for its focus target: registered before the rows, after the header.
    let h = if count == 0 { row_h } else { (rows.len() as f32 * (row_h + ui.spacing().item_spacing.y)).min(max_h) };
    let area_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), h));
    let focus = crate::ui::wip_keys::target(ui, staged, area_rect, &title);
    crate::ui::wip_keys::arrows(ui, staged, msgs);
    let focused = focus.has_focus();
    let area = egui::ScrollArea::vertical().id_salt(id).max_height(max_h).auto_shrink([false, true]);
    if count == 0 {
        let shown = area.show(ui, |ui| ui.label(RichText::new(t.tr("details.wip.none")).color(color::faint())));
        if focused {
            crate::ui::widgets::file_row_focus(ui, shown_rect(&shown));
        }
        return;
    }
    // The row showing the file `t` opens.
    let index = |t: &DiffTarget| {
        let path = match t {
            DiffTarget::Staged(p) if staged => p,
            DiffTarget::Unstaged(p) if !staged => p,
            _ => return None,
        };
        rows.row_of_file((0..count).find(|&i| entry(i).path == *path)?)
    };
    // Found in the whole list, so a current row scrolled out of view doesn't ring the list.
    let current = open.diff.and_then(index);
    let ctx = RowContext { keyed: true, ..ctx };
    // Only the rows in view are drawn: a working tree can have thousands of changed files.
    let shown = area.show_rows(ui, row_h, rows.len(), |ui, range| {
        crate::ui::details::file_reveal::reveal(ui, id, want, index, &range, row_h);
        for row in range {
            let r = match rows.get(row) {
                Shown::File { index, depth } => {
                    let ctx = RowContext { depth, ..ctx };
                    wip_file_row(ui, snap, entry(index), staged, open, ctx, None, msgs)
                }
                Shown::Folder(folder) => {
                    let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str| {
                        // Staging a conflicted file marks it resolved: refused for its folder.
                        let stage = if staged {
                            FolderStage::Unstage
                        } else {
                            let conflicted = snap.unstaged.iter().any(|f| f.status == '!' && under(path, &f.path));
                            FolderStage::Stage { blocked: conflicted.then(|| t.tr("files.folder_menu.stage_conflicted")) }
                        };
                        folder_menu::items(ui, t, msgs, path, None, stage);
                    };
                    folder_row(ui, folder, true, msgs, list, menu)
                }
            };
            if r.clicked() {
                crate::ui::wip_keys::request(ui.ctx(), staged);
            }
            if focused && current == Some(row) {
                crate::ui::widgets::file_row_focus(ui, r.rect);
            }
        }
    });
    if focused && current.is_none() {
        crate::ui::widgets::file_row_focus(ui, shown_rect(&shown));
    }
}

/// The part of a scroll area its content fills (the viewport can be taller).
fn shown_rect<R>(out: &egui::scroll_area::ScrollAreaOutput<R>) -> egui::Rect {
    let r = out.inner_rect;
    egui::Rect::from_min_size(r.min, egui::vec2(r.width(), out.content_size.y.min(r.height())))
}
