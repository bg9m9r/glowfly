//! File actions added to file rows' context menus: the file's history and blame, restore a file from a commit (one commit's
//! rows, or the rows comparing two selected commits) or revert that change to it, open it in its default program, the
//! external editor or the external diff tool (or resolve it in the merge tool), show it in the
//! file manager, start a new file in its folder, (working-tree rows) rename or delete it and,
//! for files in the working tree, track them in Git LFS, the Stash dialog's entries (the
//! Staged Files header's menu, Stash this file… on working-tree rows) and the Create patch
//! entries (a commit row's file, a working-tree row, the staged changes). Menu bodies only, each
//! drawing into the open menu's `Ui`, so tests can render them without opening a popup. `file_buttons` reuses the labels, reasons and restore message for the same
//! actions as header buttons.

use super::common::{menu_item, menu_item_if, short};
use crate::lfs::LfsMsg;
use crate::msg::BlameMsg;
use crate::msg::FilesMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::msg::StageMsg;
use crate::msg::StashMsg;
use crate::msg::WorkFileMsg;
use crate::patch::PatchMsg;
use crate::state::files::folder_prefill;
pub(super) use crate::state::lfs::TRACK_IN_LFS;
use crate::state::lfs::TrackItem;
pub(super) use crate::state::tool_compare::OPEN_IN_DIFF_TOOL;
#[cfg(test)]
pub(super) use crate::state::tool_compare::RESOLVE_IN_MERGE_TOOL;
pub(super) use crate::state::tools::OPEN_IN_EDITOR;
use crate::tools::ToolsMsg;
use eframe::egui;
use git2::Oid;
use trellis_core::patch_ops::WipScope;
use trellis_core::repo::{DiffTarget, FileEntry, Snapshot};
use trellis_core::stash_ops::StashScope;

pub(super) use crate::state::files::SHOW_IN_FOLDER;
pub(super) const OPEN_DEFAULT: &str = "Open with default app";
pub(super) const FILE_HISTORY: &str = "File history";
pub(super) const BLAME: &str = "Blame";
pub(super) const FOLDER_HISTORY: &str = "History of this folder";
pub(super) const NOT_IN_WORKDIR: &str = "Not in your working tree (deleted, renamed or never checked out)";
const DELETED_IN_WORKDIR: &str = "Not in your working tree (deleted)";
const CONFLICTED: &str = "Resolve the conflict first";
pub(super) const NEW_FILE_HERE: &str = "New file in this folder…";
pub(super) const RENAME: &str = "Rename…";
pub(super) const DELETE: &str = "Delete…";
pub(super) const UNSTAGE_DELETE: &str = "Unstage and delete…";
pub(super) const STASH_STAGED: &str = "Stash staged changes…";
pub(super) const STASH_FILE: &str = "Stash this file…";
pub(super) const PATCH_FILE: &str = "Create patch from this file…";
pub(super) const PATCH_STAGED: &str = "Create patch from staged changes…";
pub(super) const REVERT_FILE: &str = "Revert changes to this file";

/// Where a commit file row's change comes from.
#[derive(Clone, Copy)]
pub(super) enum RowSource {
    /// One commit's change.
    Commit(Oid),
    /// The change from `older` to `newer` (two selected commits).
    Between { older: Oid, newer: Oid },
}

/// File history of `path` (from commit `at`, or HEAD), then Blame of it there, disabled with the
/// reason `blame_blocked` when set, then the history of its folder (for a path in one).
pub(super) fn history_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str, at: Option<Oid>, blame_blocked: Option<&str>) {
    menu_item(msgs, ui, FILE_HISTORY, Msg::History(HistoryMsg::OpenFileHistory { path: path.to_owned(), at }));
    menu_item_if(msgs, ui, BLAME, blame_blocked, Msg::Blame(BlameMsg::Open { path: path.to_owned(), at }));
    if let Some(dir) = parent_folder(path) {
        menu_item(msgs, ui, FOLDER_HISTORY, Msg::History(HistoryMsg::OpenFolderHistory { path: dir, at }));
    }
}

/// The folder holding `path`, without a trailing '/'; None at the repository's top.
pub(super) fn parent_folder(path: &str) -> Option<String> {
    let dir = folder_prefill(path);
    Some(dir.trim_end_matches('/').to_owned()).filter(|d| !d.is_empty())
}

/// The restore action's label, for a file the commit `deleted` or one it kept.
pub(super) fn restore_label(deleted: bool) -> &'static str {
    if deleted { "Restore the deleted file…" } else { "Restore this version…" }
}

/// Asks to restore `path` from `oid` (from before it when `status` is 'D'), through the
/// confirmation.
pub(super) fn restore_msg(oid: Oid, path: &str, status: char) -> Msg {
    Msg::Files(FilesMsg::RestoreFile { oid, path: path.to_owned(), deleted: status == 'D', confirmed: false, overwrite: false })
}

/// Restore `path` from `oid`; for a file the commit deleted (`status` 'D'), from before it.
pub(super) fn restore_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, oid: Oid, path: &str, status: char) {
    menu_item(msgs, ui, restore_label(status == 'D'), restore_msg(oid, path, status));
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
        msgs.push(Msg::Files(FilesMsg::RestoreFile { oid, path: f.path.clone(), deleted: false, confirmed: false, overwrite: false }));
        ui.close_menu();
    }
}

/// Revert changes to this file: undoes `target`'s change (a commit's, or between two commits) to
/// the file in the working copy; disabled with the reason `blocked`.
pub(super) fn revert_file_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, target: DiffTarget, blocked: Option<&str>) {
    let hover = match target {
        DiffTarget::Between(..) => "Undo the changes between these commits to the file in your working copy.",
        _ => "Undo this commit's changes to the file in your working copy. The commit itself is not changed.",
    };
    let r = ui.add_enabled(blocked.is_none(), egui::Button::new(REVERT_FILE));
    let r = match blocked {
        Some(why) => r.on_disabled_hover_text(why),
        None => r.on_hover_text(hover),
    };
    if r.clicked() {
        msgs.push(Msg::Stage(StageMsg::RevertFile(target)));
        ui.close_menu();
    }
}

/// An external diff or merge tool entry: its label, what it sends, and why it is disabled.
pub(super) struct ToolItem<'a> {
    pub label: &'static str,
    pub msg: Msg,
    pub blocked: Option<&'a str>,
}

/// Open in external diff tool for `target`, disabled with `blocked`.
pub(super) fn diff_item(target: DiffTarget, blocked: Option<&str>) -> ToolItem<'_> {
    ToolItem { label: OPEN_IN_DIFF_TOOL, msg: Msg::Tools(ToolsMsg::OpenDiff(target)), blocked }
}

fn tool_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, item: ToolItem) {
    menu_item_if(msgs, ui, item.label, item.blocked, item.msg);
}

/// Open with default app, open in the external editor, the `tool` entry (with its own reason
/// when disabled: it needs no working file) and show in folder, the others disabled with
/// `missing` as the reason.
pub(super) fn open_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str, missing: Option<&str>, tool: Option<ToolItem>) {
    menu_item_if(msgs, ui, OPEN_DEFAULT, missing, Msg::Files(FilesMsg::OpenWorkingFile(path.to_owned())));
    menu_item_if(msgs, ui, OPEN_IN_EDITOR, missing, Msg::Tools(ToolsMsg::OpenFile { path: path.to_owned(), line: None }));
    if let Some(item) = tool {
        tool_item(ui, msgs, item);
    }
    menu_item_if(msgs, ui, SHOW_IN_FOLDER, missing, Msg::Files(FilesMsg::ShowWorkingFile(path.to_owned())));
}

/// Opens the New file dialog prefilled with `path`'s folder. Always enabled: a sibling never
/// touches the row's file, and missing folders are created.
fn new_file_here(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str) {
    menu_item(msgs, ui, NEW_FILE_HERE, Msg::WorkFile(WorkFileMsg::NewFileDialog(folder_prefill(path))));
}

/// The Track in Git LFS submenu's entries: each disabled with its reason, or showing on hover
/// what it writes.
pub(super) fn lfs_track_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, items: &[TrackItem]) {
    for item in items {
        let r = ui.add_enabled(item.blocked.is_none(), egui::Button::new(&item.label));
        let r = match &item.blocked {
            Some(why) => r.on_disabled_hover_text(why),
            None => r.on_hover_text(&item.hover),
        };
        if r.clicked() {
            msgs.push(Msg::Lfs(LfsMsg::TrackPath(item.target.clone())));
            ui.close_menu();
        }
    }
}

/// Separator and the Track in Git LFS submenu, unless there is nothing to offer.
fn lfs_submenu(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, items: &[TrackItem]) {
    if !items.is_empty() {
        ui.separator();
        ui.menu_button(TRACK_IN_LFS, |ui| lfs_track_items(ui, msgs, items));
    }
}

/// Extra items for a commit's file row, or a row comparing two commits: separator, restore,
/// revert changes to this file (disabled unless `in_workdir`, or the file was deleted), (one commit's row) Create patch from this file… (disabled with `patch_blocked`), open items (disabled unless `in_workdir`; Open in external diff tool disabled with
/// `diff_blocked`), separator and new file in this folder, then (only `in_workdir`) the Track
/// in Git LFS submenu with `lfs`.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_file_items(
    ui: &mut egui::Ui,
    msgs: &mut Vec<Msg>,
    src: RowSource,
    f: &FileEntry,
    in_workdir: bool,
    lfs: &[TrackItem],
    diff_blocked: Option<&str>,
    patch_blocked: Option<&str>,
) {
    ui.separator();
    let revert_blocked = (!in_workdir && f.status != 'D').then_some(NOT_IN_WORKDIR);
    let target = match src {
        RowSource::Commit(oid) => {
            restore_item(ui, msgs, oid, &f.path, f.status);
            revert_file_item(ui, msgs, DiffTarget::Commit(oid, f.path.clone()), revert_blocked);
            let m = Msg::Patch(PatchMsg::ExportFiles { oid, paths: vec![f.path.clone()] });
            menu_item_if(msgs, ui, PATCH_FILE, patch_blocked, m);
            DiffTarget::Commit(oid, f.path.clone())
        }
        RowSource::Between { older, newer } => {
            between_restore_item(ui, msgs, older, newer, f);
            revert_file_item(ui, msgs, DiffTarget::Between(older, newer, f.path.clone()), revert_blocked);
            DiffTarget::Between(older, newer, f.path.clone())
        }
    };
    open_items(ui, msgs, &f.path, (!in_workdir).then_some(NOT_IN_WORKDIR), Some(diff_item(target, diff_blocked)));
    ui.separator();
    new_file_here(ui, msgs, &f.path);
    if in_workdir {
        lfs_submenu(ui, msgs, lfs);
    }
}

/// Extra items for a working-tree row: separator, open items (disabled when `missing`, with
/// `tool` among them: the row's diff tool entry, or the merge tool's for a conflicted row), then
/// separator, new file in this folder, rename and delete (the last two also disabled while the
/// row is conflicted), then (unless `missing`) the Track in Git LFS submenu with `lfs`. `unstages` means deleting this path also takes it out of the index (staged
/// as new or as a rename's new path): the item then reads "Unstage and delete…" and stays enabled
/// when the file is already gone, since unstaging it is still useful.
pub(super) fn wip_file_items(
    ui: &mut egui::Ui,
    msgs: &mut Vec<Msg>,
    f: &FileEntry,
    missing: bool,
    unstages: bool,
    lfs: &[TrackItem],
    tool: ToolItem,
) {
    ui.separator();
    let gone = missing.then_some(DELETED_IN_WORKDIR);
    open_items(ui, msgs, &f.path, gone, Some(tool));
    ui.separator();
    new_file_here(ui, msgs, &f.path);
    let conflicted = (f.status == '!').then_some(CONFLICTED);
    let blocked = gone.or(conflicted);
    menu_item_if(msgs, ui, RENAME, blocked, Msg::WorkFile(WorkFileMsg::RenameFileDialog(f.path.clone())));
    let (label, del_blocked) = if unstages { (UNSTAGE_DELETE, conflicted) } else { (DELETE, blocked) };
    menu_item_if(msgs, ui, label, del_blocked, Msg::WorkFile(WorkFileMsg::DeleteFile { path: f.path.clone(), confirmed: false }));
    if !missing {
        lfs_submenu(ui, msgs, lfs);
    }
}

/// The Staged Files header's menu: open the Stash dialog on the staged changes (disabled while
/// nothing is staged or a file is conflicted), the Create patch dialog on them, and unstage
/// everything.
pub(super) fn staged_header_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, snap: &Snapshot) {
    let blocked = crate::state::stashes::push_blocked(snap, StashScope::Staged, &[]);
    menu_item_if(msgs, ui, STASH_STAGED, blocked, Msg::Stash(StashMsg::PushDialog { scope: StashScope::Staged, path: None }));
    let nothing = snap.staged.is_empty().then_some("Nothing is staged");
    let m = Msg::Patch(PatchMsg::ChangesDialog { scope: WipScope::Staged, path: None });
    menu_item_if(msgs, ui, PATCH_STAGED, nothing, m);
    menu_item_if(msgs, ui, "Unstage all", nothing, Msg::Stage(StageMsg::UnstageAll));
}

/// Opens the Create patch dialog on this working-tree row alone (its staged or unstaged
/// changes); none for a conflicted row, disabled for a row Trellis can't name safely.
pub(super) fn patch_file_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, f: &FileEntry, staged: bool) {
    if f.status == '!' {
        return;
    }
    let scope = if staged { WipScope::Staged } else { WipScope::Unstaged };
    let m = Msg::Patch(PatchMsg::ChangesDialog { scope, path: Some(f.path.clone()) });
    menu_item_if(msgs, ui, PATCH_FILE, f.hazard.as_ref().map(|_| "Trellis can't name this path to git safely"), m);
}

/// Opens the Stash dialog on Selected files with only `f` ticked; disabled while any file is
/// conflicted, since git can't stash then, and for a row Trellis can't stash safely.
pub(super) fn stash_file_item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, snap: &Snapshot, f: &FileEntry) {
    let conflicted = snap.unstaged.iter().any(|f| f.status == '!').then_some("Resolve the conflicted files first");
    let unsafe_path = f.hazard.as_ref().map(|_| "Trellis can't stash this path safely");
    let m = Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, path: Some(f.path.clone()) });
    menu_item_if(msgs, ui, STASH_FILE, conflicted.or(unsafe_path), m);
}
