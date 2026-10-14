//! The WORKTREES section of the left panel: one row per worktree (the main one first) with what
//! it has checked out, whether it is locked or missing, and a tooltip. A click jumps to its HEAD
//! commit, a double-click opens it in a tab, and the row menu opens it, shows its folder, copies
//! its path, locks or unlocks it, removes it, and prunes missing records (also from the header's
//! menu).

use super::common::{menu_item, menu_item_if};
use super::theme::color;
use super::widgets;
use crate::msg::Msg;
use crate::state::Selection;
use crate::state::worktree_manage::{manage_blocked, prune_blocked, remove_blocked, row_prune_blocked};
use crate::state::worktrees::{open_blocked, right_text, tooltip};
use crate::worktree::WorktreeMsg;
use eframe::egui;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::repo::{Snapshot, WorktreeInfo};

/// What the rows are drawn against: the left-panel filter, the selection, and the folders open
/// in tabs (Remove… is disabled for those).
pub(super) struct Rows<'a> {
    pub shown: &'a dyn Fn(&str) -> bool,
    pub sel: &'a Selection,
    pub open_tabs: &'a [PathBuf],
}

pub(super) fn section(ui: &mut egui::Ui, snap: &Snapshot, open: &mut bool, rows: Rows, hover_next: &mut Option<Oid>, msgs: &mut Vec<Msg>) {
    let Rows { shown, sel, open_tabs } = rows;
    let h = widgets::section_header(ui, "WORKTREES", Some(snap.worktrees.len()), open);
    h.context_menu(|ui| menu_item_if(msgs, ui, "Prune missing worktrees", prune_blocked(snap), Msg::Worktree(WorktreeMsg::Prune)));
    if !*open {
        return;
    }
    let matches =
        |w: &WorktreeInfo| shown(&w.folder_name()) || w.branch.as_deref().is_some_and(shown) || shown(&w.path.display().to_string());
    for wt in snap.worktrees.iter().filter(|w| matches(w)) {
        let label = if wt.current { format!("✔  {}", wt.folder_name()) } else { wt.folder_name() };
        let col = if wt.locked.is_some() {
            color::amber()
        } else if wt.missing {
            color::red()
        } else {
            color::weak()
        };
        let selected = wt.head.is_some_and(|o| *sel == Selection::Commit(o));
        let right = right_text(wt);
        let r = widgets::list_row(ui, 26.0, &label, Some((&right, col)), selected, wt.current);
        if r.hovered() {
            *hover_next = wt.head;
        }
        let r = r.on_hover_text(tooltip(wt));
        if r.double_clicked() {
            if open_blocked(wt).is_none() {
                msgs.push(Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
            }
        } else if r.clicked()
            && let Some(o) = wt.head
        {
            msgs.push(Msg::JumpTo(o));
        }
        r.context_menu(|ui| menu(ui, wt, open_tabs, msgs));
    }
}

fn menu(ui: &mut egui::Ui, wt: &WorktreeInfo, open_tabs: &[PathBuf], msgs: &mut Vec<Msg>) {
    menu_item_if(msgs, ui, "Open in new tab", open_blocked(wt), Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
    let gone = wt.missing.then_some("Its folder is missing.");
    menu_item_if(msgs, ui, "Open folder", gone, Msg::OpenFolder(wt.path.clone()));
    menu_item(msgs, ui, "Copy path", Msg::Copy(wt.path.display().to_string()));
    // git refuses all of these for the main worktree.
    if wt.is_main() {
        return;
    }
    ui.separator();
    let path = wt.path.clone();
    if wt.locked.is_some() {
        menu_item_if(msgs, ui, "Unlock", manage_blocked(wt), Msg::Worktree(WorktreeMsg::Unlock(path.clone())));
    } else {
        menu_item_if(msgs, ui, "Lock…", manage_blocked(wt), Msg::Worktree(WorktreeMsg::LockDialog(path.clone())));
    }
    menu_item_if(msgs, ui, "Remove…", remove_blocked(wt, open_tabs), Msg::Worktree(WorktreeMsg::RemoveDialog(path)));
    if wt.missing {
        menu_item_if(msgs, ui, "Prune missing worktrees", row_prune_blocked(wt), Msg::Worktree(WorktreeMsg::Prune));
    }
}
