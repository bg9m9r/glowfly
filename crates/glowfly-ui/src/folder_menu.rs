//! A folder row's menu in the changed-file lists' tree: its history, its path, and in the WIP
//! lists staging or unstaging everything under it.

use crate::msg::{HistoryMsg, Msg, StageMsg};
use crate::ui::common::{menu_item, menu_item_if};
use eframe::egui;
use git2::Oid;
use glowfly_core::i18n::Lang;

/// What the menu offers for the folder's files beside history and path.
#[derive(Clone, Copy)]
pub(crate) enum FolderStage<'a> {
    /// A commit's or a comparison's list: nothing to stage.
    None,
    /// Unstaged Files: Stage folder, disabled with `blocked` while a conflicted file is under it.
    Stage { blocked: Option<&'a str> },
    /// Staged Files: Unstage folder.
    Unstage,
}

/// The items for the folder `path` (no trailing '/'), its history seen from `at` (None: HEAD).
pub(crate) fn items(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, path: &str, at: Option<Oid>, stage: FolderStage) {
    let history = Msg::History(HistoryMsg::OpenFolderHistory { path: path.to_owned(), at });
    menu_item(msgs, ui, t.tr("files.folder_menu.history"), history);
    menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(path.to_owned()));
    match stage {
        FolderStage::None => {}
        FolderStage::Stage { blocked } => {
            let msg = Msg::Stage(StageMsg::StageFolder(path.to_owned()));
            menu_item_if(msgs, ui, t.tr("files.folder_menu.stage"), blocked, msg);
        }
        FolderStage::Unstage => {
            menu_item(msgs, ui, t.tr("files.folder_menu.unstage"), Msg::Stage(StageMsg::UnstageFolder(path.to_owned())));
        }
    }
}
