//! File actions added to file rows' context menus: restore a file from a commit (one commit's
//! rows, or the rows comparing two selected commits), open it in its default program, show it in
//! the file manager, start a new file in its folder, and (working-tree rows) rename or delete it.
//! Menu bodies only, each drawing into the open menu's `Ui`, so tests can render them without
//! opening a popup.

use super::common::{menu_item, menu_item_if, short};
use crate::msg::Msg;
use crate::state::files::folder_prefill;
use eframe::egui;
use git2::Oid;
use trellis_core::repo::FileEntry;

/// The show-in-file-manager label for this platform.
pub(super) const SHOW_IN_FOLDER: &str = if cfg!(windows) {
    "Show in Explorer"
} else if cfg!(target_os = "macos") {
    "Reveal in Finder"
} else {
    "Open containing folder"
};
pub(super) const OPEN_DEFAULT: &str = "Open with default app";
const NOT_IN_WORKDIR: &str = "Not in your working tree (deleted, renamed or never checked out)";
const DELETED_IN_WORKDIR: &str = "Not in your working tree (deleted)";
const CONFLICTED: &str = "Resolve the conflict first";
pub(super) const NEW_FILE_HERE: &str = "New file in this folder…";
pub(super) const RENAME: &str = "Rename…";
pub(super) const DELETE: &str = "Delete…";
pub(super) const UNSTAGE_DELETE: &str = "Unstage and delete…";

/// Where a commit file row's change comes from.
#[derive(Clone, Copy)]
pub(super) enum RowSource {
    /// One commit's change.
    Commit(Oid),
    /// The change from `older` to `newer` (two selected commits).
    Between { older: Oid, newer: Oid },
}

/// Restore `path` from `oid`; for a file the commit deleted (`status` 'D'), from before it.
pub(super) fn restore_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, oid: Oid, path: &str, status: char) {
    let deleted = status == 'D';
    let label = if deleted { "Restore the deleted file…" } else { "Restore this version…" };
    menu_item(msgs, ui, label, Msg::RestoreFile { oid, path: path.to_owned(), deleted, confirmed: false, overwrite: false });
}

/// Restore a comparison row's file from the newer commit, or from the older one when the newer
/// commit doesn't have it (status 'D'). The newer commit's first parent is not used for that
/// case: it can be any commit, unrelated to this comparison, and may not have the file at all.
fn between_restore_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, older: Oid, newer: Oid, f: &FileEntry) {
    let gone = f.status == 'D';
    let oid = if gone { older } else { newer };
    let mut r = ui.button(format!("Restore the version from {}…", short(oid)));
    if gone {
        r = r.on_hover_text(format!("{} doesn't have this file, so this brings back its version from {}.", short(newer), short(older)));
    }
    if r.clicked() {
        msgs.push(Msg::RestoreFile { oid, path: f.path.clone(), deleted: false, confirmed: false, overwrite: false });
        ui.close_menu();
    }
}

/// Open with default app and show in folder, disabled with `missing` as the reason.
pub(super) fn open_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str, missing: Option<&str>) {
    menu_item_if(msgs, ui, OPEN_DEFAULT, missing, Msg::OpenWorkingFile(path.to_owned()));
    menu_item_if(msgs, ui, SHOW_IN_FOLDER, missing, Msg::ShowWorkingFile(path.to_owned()));
}

/// Opens the New file dialog prefilled with `path`'s folder. Always enabled: a sibling never
/// touches the row's file, and missing folders are created.
fn new_file_here(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str) {
    menu_item(msgs, ui, NEW_FILE_HERE, Msg::NewFileDialog(folder_prefill(path)));
}

/// Extra items for a commit's file row, or a row comparing two commits: separator, restore,
/// open items (disabled unless `in_workdir`), then separator and new file in this folder.
pub(super) fn commit_file_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, src: RowSource, f: &FileEntry, in_workdir: bool) {
    ui.separator();
    match src {
        RowSource::Commit(oid) => restore_item(ui, msgs, oid, &f.path, f.status),
        RowSource::Between { older, newer } => between_restore_item(ui, msgs, older, newer, f),
    }
    open_items(ui, msgs, &f.path, (!in_workdir).then_some(NOT_IN_WORKDIR));
    ui.separator();
    new_file_here(ui, msgs, &f.path);
}

/// Extra items for a working-tree row: separator, open items (disabled when `missing`), then
/// separator, new file in this folder, rename and delete (the last two also disabled while the
/// row is conflicted). `unstages` means deleting this path also takes it out of the index (staged
/// as new or as a rename's new path): the item then reads "Unstage and delete…" and stays enabled
/// when the file is already gone, since unstaging it is still useful.
pub(super) fn wip_file_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, f: &FileEntry, missing: bool, unstages: bool) {
    ui.separator();
    let gone = missing.then_some(DELETED_IN_WORKDIR);
    open_items(ui, msgs, &f.path, gone);
    ui.separator();
    new_file_here(ui, msgs, &f.path);
    let conflicted = (f.status == '!').then_some(CONFLICTED);
    let blocked = gone.or(conflicted);
    menu_item_if(msgs, ui, RENAME, blocked, Msg::RenameFileDialog(f.path.clone()));
    let (label, del_blocked) = if unstages { (UNSTAGE_DELETE, conflicted) } else { (DELETE, blocked) };
    menu_item_if(msgs, ui, label, del_blocked, Msg::DeleteFile { path: f.path.clone(), confirmed: false });
}
