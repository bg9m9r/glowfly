//! The file row menus' Track in Git LFS submenu, the Stash dialog's entries (the Staged Files
//! header's menu, Stash this file… on working-tree rows) and the Create patch entries of
//! working-tree rows (`file_menu.rs` holds the rest).

use super::super::common::menu_item_if;
use super::super::widgets::reason::{self, ItemState};
use crate::lfs::LfsMsg;
use crate::msg::{Msg, StageMsg, StashMsg};
use crate::patch::PatchMsg;
use crate::state::lfs::TrackItem;
use crate::state::stashes::push_blocked;
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_core::patch_ops::WipScope;
use glowfly_core::repo::{FileEntry, Snapshot};
use glowfly_core::stash_ops::StashScope;

/// The Track in Git LFS submenu's entries: each disabled with its reason, or showing on hover
/// what it writes.
pub(in crate::ui) fn lfs_track_items(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, items: &[TrackItem]) {
    for item in items {
        let r = match &item.blocked {
            Some(why) => reason::menu_button(ui, &item.label, "", ItemState::Blocked(why.as_str())),
            None => reason::menu_button(ui, &item.label, "", ItemState::Ready).on_hover_text(&item.hover),
        };
        if r.clicked() {
            msgs.push(Msg::Lfs(LfsMsg::TrackPath(item.target.clone())));
            ui.close_menu();
        }
    }
}

/// Separator and the Track in Git LFS submenu, unless there is nothing to offer.
pub(super) fn lfs_submenu(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, items: &[TrackItem]) {
    if !items.is_empty() {
        ui.separator();
        ui.menu_button(t.tr("lfs.menu.track"), |ui| lfs_track_items(ui, msgs, items));
    }
}

/// The Staged Files header's menu: open the Stash dialog on the staged changes (disabled while
/// nothing is staged or a file is conflicted), the Create patch dialog on them, and unstage
/// everything.
pub(in crate::ui) fn staged_header_items(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, snap: &Snapshot) {
    let blocked = push_blocked(t, snap, StashScope::Staged, &[]);
    let stash = Msg::Stash(StashMsg::PushDialog { scope: StashScope::Staged, path: None });
    menu_item_if(msgs, ui, t.tr("files.menu.stash_staged"), blocked, stash);
    let nothing = snap.staged.is_empty().then(|| t.tr("files.menu.nothing_staged"));
    let m = Msg::Patch(PatchMsg::ChangesDialog { scope: WipScope::Staged, path: None });
    menu_item_if(msgs, ui, t.tr("files.menu.patch_staged"), nothing, m);
    menu_item_if(msgs, ui, t.tr("details.wip.unstage_all.button"), nothing, Msg::Stage(StageMsg::UnstageAll));
}

/// Opens the Create patch dialog on this working-tree row alone (its staged or unstaged
/// changes); none for a conflicted row, disabled for a row Glowfly can't name safely.
pub(in crate::ui) fn patch_file_item(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, f: &FileEntry, staged: bool) {
    if f.status == '!' {
        return;
    }
    let scope = if staged { WipScope::Staged } else { WipScope::Unstaged };
    let m = Msg::Patch(PatchMsg::ChangesDialog { scope, path: Some(f.path.clone()) });
    menu_item_if(msgs, ui, t.tr("files.menu.patch_file"), f.hazard.as_ref().map(|_| t.tr("files.menu.unsafe_name")), m);
}

/// Opens the Stash dialog on Selected files with only `f` ticked; disabled while any file is
/// conflicted, since git can't stash then, and for a row Glowfly can't stash safely.
pub(in crate::ui) fn stash_file_item(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, snap: &Snapshot, f: &FileEntry) {
    let conflicted = snap.unstaged.iter().any(|f| f.status == '!').then(|| t.tr("files.menu.resolve_conflicted"));
    let unsafe_path = f.hazard.as_ref().map(|_| t.tr("files.menu.unsafe_stash"));
    let m = Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, path: Some(f.path.clone()) });
    menu_item_if(msgs, ui, t.tr("files.menu.stash_file"), conflicted.or(unsafe_path), m);
}
