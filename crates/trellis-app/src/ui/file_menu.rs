//! File actions added to file rows' context menus: restore a file from a commit, open it in its
//! default program, show it in the file manager, and (working-tree rows) rename or delete it.
//! Menu bodies only, each drawing into the open menu's `Ui`, so tests can render them without
//! opening a popup.

use super::common::{menu_item, menu_item_if};
use crate::msg::Msg;
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
pub(super) const RENAME: &str = "Rename…";
pub(super) const DELETE: &str = "Delete…";

/// Restore `path` from `oid`; for a file the commit deleted (`status` 'D'), from before it.
pub(super) fn restore_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, oid: Oid, path: &str, status: char) {
    let deleted = status == 'D';
    let label = if deleted { "Restore the deleted file…" } else { "Restore this version…" };
    menu_item(msgs, ui, label, Msg::RestoreFile { oid, path: path.to_owned(), deleted, confirmed: false, overwrite: false });
}

/// Open with default app and show in folder, disabled with `missing` as the reason.
pub(super) fn open_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str, missing: Option<&str>) {
    menu_item_if(msgs, ui, OPEN_DEFAULT, missing, Msg::OpenWorkingFile(path.to_owned()));
    menu_item_if(msgs, ui, SHOW_IN_FOLDER, missing, Msg::ShowWorkingFile(path.to_owned()));
}

/// Extra items for a commit's file row: separator, restore, open items.
pub(super) fn commit_file_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, oid: Oid, f: &FileEntry, in_workdir: bool) {
    ui.separator();
    restore_item(ui, msgs, oid, &f.path, f.status);
    open_items(ui, msgs, &f.path, (!in_workdir).then_some(NOT_IN_WORKDIR));
}

/// Extra items for a working-tree row: separator, open items (disabled when `missing`), then
/// separator, rename and delete (also disabled while the row is conflicted).
pub(super) fn wip_file_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, f: &FileEntry, missing: bool) {
    ui.separator();
    let gone = missing.then_some(DELETED_IN_WORKDIR);
    open_items(ui, msgs, &f.path, gone);
    ui.separator();
    let blocked = gone.or((f.status == '!').then_some(CONFLICTED));
    menu_item_if(msgs, ui, RENAME, blocked, Msg::RenameFileDialog(f.path.clone()));
    menu_item_if(msgs, ui, DELETE, blocked, Msg::DeleteFile { path: f.path.clone(), confirmed: false });
}
