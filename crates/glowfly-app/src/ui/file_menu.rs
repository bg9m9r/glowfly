//! File actions added to file rows' context menus: the file's history and blame, restore a file from a commit (one commit's
//! rows, the rows comparing selected commits, or a commit with the working tree) or revert a
//! commit's or comparison's change to it, open it in its default program, the
//! external editor or the external diff tool (or resolve it in the merge tool), show it in the
//! file manager, start a new file in its folder, (working-tree rows) rename or delete it and,
//! for files in the working tree, track them in Git LFS, the Stash dialog's entries (the
//! Staged Files header's menu, Stash this file… on working-tree rows) and the Create patch
//! entries (a commit row's file, a working-tree row, the staged changes; the LFS, stash and
//! patch items are in `file_menu_extra.rs`), then the custom commands (`custom_menu.rs`). Menu bodies only, each drawing into the open
//! menu's `Ui`, so tests can render them without opening a popup. `file_buttons` reuses the
//! restore label and message for the same actions as header buttons.

#[path = "folder_menu.rs"]
pub(super) mod folder_menu;

#[path = "file_menu_extra.rs"]
mod extra;

#[cfg(test)]
pub(super) use extra::lfs_track_items;
pub(super) use extra::{patch_file_item, staged_header_items, stash_file_item};

use super::common::{menu_item, menu_item_if, menu_item_setup, short};
use super::widgets::reason::{self, ItemState};
use crate::custom_commands::Target;
use crate::msg::BlameMsg;
use crate::msg::FilesMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::msg::StageMsg;
use crate::msg::WorkFileMsg;
use crate::patch::PatchMsg;
use crate::state::custom_commands::MenuCommands;
use crate::state::files::{folder_prefill, not_in_workdir, show_in_folder};
use crate::state::lfs::TrackItem;
use crate::state::tool_compare::ToolSetup;
use crate::state::tools::open_in_editor;
use crate::tools::ToolsMsg;
use eframe::egui;
use git2::Oid;
use glowfly_core::custom_commands::Place;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, FileEntry, is_empty_tree};

// In English for the file history's views, which aren't translated yet; the menus use keys.
pub(super) use crate::state::files::NOT_IN_WORKDIR;
pub(super) use crate::state::files::SHOW_IN_FOLDER;
pub(super) const OPEN_DEFAULT: &str = "Open with default app";
pub(super) const BLAME: &str = "Blame";

/// Where a commit file row's change comes from.
#[derive(Clone, Copy)]
pub(super) enum RowSource {
    /// One commit's change.
    Commit(Oid),
    /// The change from commit `from` to commit `to` (two selected commits, a line of them, or a
    /// pull request's).
    Between { from: Oid, to: Oid },
    /// The change from this commit to the working tree.
    ToWorkdir(Oid),
    /// The change from the working tree to this commit (the swapped comparison).
    FromWorkdir(Oid),
}

/// File history of `path` (from commit `at`, or HEAD), then Blame of it there, disabled with the
/// reason `blame_blocked` when set, then the history of its folder (for a path in one).
pub(super) fn history_items(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, path: &str, at: Option<Oid>, blame_blocked: Option<&str>) {
    let history = Msg::History(HistoryMsg::OpenFileHistory { path: path.to_owned(), at });
    menu_item(msgs, ui, t.tr("files.menu.file_history"), history);
    menu_item_if(msgs, ui, t.tr("files.menu.blame"), blame_blocked, Msg::Blame(BlameMsg::Open { path: path.to_owned(), at }));
    if let Some(dir) = parent_folder(path) {
        menu_item(msgs, ui, t.tr("files.menu.folder_history"), Msg::History(HistoryMsg::OpenFolderHistory { path: dir, at }));
    }
}

/// The folder holding `path`, without a trailing '/'; None at the repository's top.
pub(super) fn parent_folder(path: &str) -> Option<String> {
    let dir = folder_prefill(path);
    Some(dir.trim_end_matches('/').to_owned()).filter(|d| !d.is_empty())
}

/// The restore action's label, for a file the commit `deleted` or one it kept.
pub(super) fn restore_label(t: Lang, deleted: bool) -> &'static str {
    if deleted { t.tr("files.menu.restore_deleted") } else { t.tr("files.menu.restore_version") }
}

/// Asks to restore `path` from `oid` (from before it when `status` is 'D'), through the
/// confirmation.
pub(super) fn restore_msg(oid: Oid, path: &str, status: char) -> Msg {
    Msg::Files(FilesMsg::RestoreFile { oid, path: path.to_owned(), deleted: status == 'D', confirmed: false, overwrite: false })
}

/// Restore `path` from `oid`; for a file the commit deleted (`status` 'D'), from before it.
pub(super) fn restore_item(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, oid: Oid, path: &str, status: char) {
    menu_item(msgs, ui, restore_label(t, status == 'D'), restore_msg(oid, path, status));
}

/// Restore a comparison row's file from the commit compared to (`to`), or from `from` when `to`
/// doesn't have it (status 'D'). `to`'s first parent is not used for that case: it can be any
/// commit, unrelated to this comparison, and may not have the file at all. When `to` is the
/// empty tree (a swapped range from the first commit) the tip says so rather than name it.
fn between_restore_item(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, from: Oid, to: Oid, f: &FileEntry) {
    let gone = f.status == 'D';
    let oid = if gone { from } else { to };
    let mut r = ui.button(t.trf("files.menu.restore_from", &[("sha", &short(oid))]));
    if gone {
        let tip = if is_empty_tree(to) {
            t.trf("files.menu.restore_from_empty.tip", &[("from", &short(from))])
        } else {
            t.trf("files.menu.restore_from.tip", &[("to", &short(to)), ("from", &short(from))])
        };
        r = r.on_hover_text(tip);
    }
    if r.clicked() {
        msgs.push(Msg::Files(FilesMsg::RestoreFile { oid, path: f.path.clone(), deleted: false, confirmed: false, overwrite: false }));
        ui.close_menu();
    }
}

/// Revert changes to this file: undoes `target`'s change (a commit's, or between two commits) to
/// the file in the working copy; disabled with the reason `blocked`.
pub(super) fn revert_file_item(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, target: DiffTarget, blocked: Option<&str>) {
    let hover = match target {
        DiffTarget::Between(..) => t.tr("files.menu.revert_between.tip"),
        _ => t.tr("files.menu.revert_file.tip"),
    };
    let label = t.tr("files.menu.revert_file");
    let r = match blocked {
        Some(why) => reason::menu_button(ui, label, "", ItemState::Blocked(why)),
        None => reason::menu_button(ui, label, "", ItemState::Ready).on_hover_text(hover),
    };
    if r.clicked() {
        msgs.push(Msg::Stage(StageMsg::RevertFile(target)));
        ui.close_menu();
    }
}

/// Restore a working-tree comparison row's file as commit `oid` has it; disabled for a file the
/// commit doesn't have (added since, or a rename's new path; 'D' in the `swapped` comparison).
fn workdir_restore_item(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, oid: Oid, f: &FileEntry, swapped: bool) {
    let lacks = if swapped { f.status == 'D' } else { matches!(f.status, 'A' | 'R') };
    let missing = lacks.then(|| t.trf("details.file.not_in_commit", &[("sha", &short(oid))]));
    let state = ItemState::blocked_by(missing.as_deref());
    let r = reason::menu_button(ui, t.trf("files.menu.restore_from", &[("sha", &short(oid))]), "", state);
    if r.clicked() {
        msgs.push(Msg::Files(FilesMsg::RestoreFile { oid, path: f.path.clone(), deleted: false, confirmed: false, overwrite: false }));
        ui.close_menu();
    }
}

/// An external diff or merge tool entry: its label, what it sends, why it is disabled, and its
/// setup note while its program isn't set (it then stays enabled and opens Preferences).
pub(super) struct ToolItem<'a> {
    pub label: &'static str,
    pub msg: Msg,
    pub blocked: Option<&'a str>,
    pub setup: Option<&'static str>,
}

/// Open in external diff tool for `target`, with "Set up…" while no tool is set (`setup`).
pub(super) fn diff_item(t: Lang, target: DiffTarget, setup: Option<&'static str>) -> ToolItem<'static> {
    ToolItem { label: t.tr("tools.open_in_diff_tool"), msg: Msg::Tools(ToolsMsg::OpenDiff(target)), blocked: None, setup }
}

fn tool_item(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, item: ToolItem) {
    match item.blocked {
        Some(why) => menu_item_if(msgs, ui, item.label, Some(why), item.msg),
        None => menu_item_setup(msgs, ui, t, item.label, item.setup, item.msg),
    }
}

/// Open with default app, open in the external editor (with "Set up…" while no editor is set:
/// `editor_setup`), the `tool` entry (with its own reason when disabled: it needs no working
/// file) and show in folder, the others disabled with `missing` as the reason.
pub(super) fn open_items(
    ui: &mut egui::Ui,
    t: Lang,
    msgs: &mut Vec<Msg>,
    path: &str,
    missing: Option<&str>,
    editor_setup: Option<&'static str>,
    tool: Option<ToolItem>,
) {
    menu_item_if(msgs, ui, t.tr("files.menu.open_default"), missing, Msg::Files(FilesMsg::OpenWorkingFile(path.to_owned())));
    let editor = Msg::Tools(ToolsMsg::OpenFile { path: path.to_owned(), line: None });
    match missing {
        Some(_) => menu_item_if(msgs, ui, open_in_editor(t), missing, editor),
        None => menu_item_setup(msgs, ui, t, open_in_editor(t), editor_setup, editor),
    }
    if let Some(item) = tool {
        tool_item(ui, t, msgs, item);
    }
    menu_item_if(msgs, ui, show_in_folder(t), missing, Msg::Files(FilesMsg::ShowWorkingFile(path.to_owned())));
}

/// Opens the New file dialog prefilled with `path`'s folder. Always enabled: a sibling never
/// touches the row's file, and missing folders are created.
fn new_file_here(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, path: &str) {
    menu_item(msgs, ui, t.tr("files.menu.new_file_here"), Msg::WorkFile(WorkFileMsg::NewFileDialog(folder_prefill(path))));
}

/// Extra items for a commit's file row, or a row comparing two commits: separator, restore,
/// revert changes to this file (disabled unless `in_workdir`, or the file was deleted), (one commit's row) Create patch from this file… (disabled with `patch_blocked`), open items (disabled unless `in_workdir`; the editor and diff tool entries
/// say "Set up…" while `tools` has no program for them), separator and new file in this folder, then (only `in_workdir`) the Track
/// in Git LFS submenu with `lfs`.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_file_items(
    ui: &mut egui::Ui,
    t: Lang,
    msgs: &mut Vec<Msg>,
    src: RowSource,
    f: &FileEntry,
    in_workdir: bool,
    lfs: &[TrackItem],
    tools: ToolSetup,
    patch_blocked: Option<&str>,
    cmds: &MenuCommands,
) {
    ui.separator();
    let missing = (!in_workdir).then(|| not_in_workdir(t));
    let revert_blocked = missing.filter(|_| f.status != 'D');
    let target = match src {
        RowSource::Commit(oid) => {
            restore_item(ui, t, msgs, oid, &f.path, f.status);
            revert_file_item(ui, t, msgs, DiffTarget::Commit(oid, f.path.clone()), revert_blocked);
            let m = Msg::Patch(PatchMsg::ExportFiles { oid, paths: vec![f.path.clone()] });
            menu_item_if(msgs, ui, t.tr("files.menu.patch_file"), patch_blocked, m);
            DiffTarget::Commit(oid, f.path.clone())
        }
        RowSource::Between { from, to } => {
            between_restore_item(ui, t, msgs, from, to, f);
            revert_file_item(ui, t, msgs, DiffTarget::Between(from, to, f.path.clone()), revert_blocked);
            DiffTarget::Between(from, to, f.path.clone())
        }
        RowSource::ToWorkdir(oid) => {
            workdir_restore_item(ui, t, msgs, oid, f, false);
            DiffTarget::ToWorkdir(oid, f.path.clone())
        }
        // No Revert changes to this file either way round: Restore covers the whole file.
        RowSource::FromWorkdir(oid) => {
            workdir_restore_item(ui, t, msgs, oid, f, true);
            DiffTarget::FromWorkdir(oid, f.path.clone())
        }
    };
    open_items(ui, t, msgs, &f.path, missing, tools.editor, Some(diff_item(t, target, tools.diff)));
    ui.separator();
    new_file_here(ui, t, msgs, &f.path);
    if in_workdir {
        extra::lfs_submenu(ui, t, msgs, lfs);
    }
    let at = match src {
        RowSource::Commit(oid) | RowSource::ToWorkdir(oid) | RowSource::FromWorkdir(oid) => oid,
        RowSource::Between { to, .. } => to,
    };
    super::custom_menu::group(ui, t, cmds, Place::File, Target::File { path: f.path.clone(), at: Some(at) }, msgs);
}

/// Extra items for a working-tree row: separator, open items (disabled when `missing`, with
/// `tool` among them: the row's diff tool entry, or the merge tool's for a conflicted row), then
/// separator, new file in this folder, rename and delete (the last two also disabled while the
/// row is conflicted), then (unless `missing`) the Track in Git LFS submenu with `lfs`. The editor
/// entry says "Set up…" while no editor is set (`editor_setup`). `unstages` means deleting this path also takes it out of the index (staged
/// as new or as a rename's new path): the item then reads "Unstage and delete…" and stays enabled
/// when the file is already gone, since unstaging it is still useful.
#[allow(clippy::too_many_arguments)]
pub(super) fn wip_file_items(
    ui: &mut egui::Ui,
    t: Lang,
    msgs: &mut Vec<Msg>,
    f: &FileEntry,
    missing: bool,
    unstages: bool,
    lfs: &[TrackItem],
    editor_setup: Option<&'static str>,
    tool: ToolItem,
    cmds: &MenuCommands,
) {
    ui.separator();
    let gone = missing.then(|| t.tr("files.menu.deleted_in_workdir"));
    open_items(ui, t, msgs, &f.path, gone, editor_setup, Some(tool));
    ui.separator();
    new_file_here(ui, t, msgs, &f.path);
    let conflicted = (f.status == '!').then(|| t.tr("files.menu.conflicted"));
    let blocked = gone.or(conflicted);
    menu_item_if(msgs, ui, t.tr("files.menu.rename"), blocked, Msg::WorkFile(WorkFileMsg::RenameFileDialog(f.path.clone())));
    let (label, del_blocked) =
        if unstages { (t.tr("files.menu.unstage_delete"), conflicted) } else { (t.tr("files.menu.delete"), blocked) };
    menu_item_if(msgs, ui, label, del_blocked, Msg::WorkFile(WorkFileMsg::DeleteFile { path: f.path.clone(), confirmed: false }));
    if !missing {
        extra::lfs_submenu(ui, t, msgs, lfs);
    }
    super::custom_menu::group(ui, t, cmds, Place::File, Target::File { path: f.path.clone(), at: None }, msgs);
}
