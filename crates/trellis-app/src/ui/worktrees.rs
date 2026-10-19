//! The WORKTREES section of the left panel: once a worktree is linked, one row per worktree (the
//! main one first) with what it has checked out, whether it is locked or missing, and a tooltip;
//! before that, an empty state line offering Add worktree…. A click jumps to its HEAD commit, a
//! double-click opens it in a tab, and the row menu (also on the worktree's WIP row in the graph)
//! opens it, shows its folder, copies its path, locks or unlocks it, moves it, removes it, and
//! prunes missing records. The header's + adds a worktree from HEAD, and its menu adds one or
//! prunes missing records.

use super::common::{menu_item, menu_item_if};
use super::theme::color;
use super::widgets;
use super::widgets::section_header::{self, Header, HeaderAdd, action_item};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::Selection;
use crate::state::section_actions::SectionAction;
use crate::state::worktree_manage::{manage_blocked, open_elsewhere, prune_blocked, remove_blocked, row_prune_blocked};
use crate::state::worktree_move::move_refused;
use crate::state::worktrees::{listed, open_blocked, right_text, tooltip};
use crate::worktree::WorktreeMsg;
use eframe::egui;
use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::repo::{Snapshot, WorktreeInfo};

/// What the rows are drawn against: the left-panel filter (and whether it has text), the
/// selection, the folders open in tabs (their tooltips say so), and the header's Add worktree.
pub(super) struct Rows<'a> {
    pub shown: &'a dyn Fn(&str) -> bool,
    pub sel: &'a Selection,
    pub open_tabs: &'a [crate::state::tabs::Place],
    pub add: &'a SectionAction,
    pub filtering: bool,
    pub t: Lang,
}

pub(super) fn section(ui: &mut egui::Ui, snap: &Snapshot, open: &mut bool, rows: Rows, hover_next: &mut Option<Oid>, msgs: &mut Vec<Msg>) {
    let Rows { shown, sel, open_tabs, add, filtering, t } = rows;
    let list = listed(snap);
    let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| {
        action_item(ui, msgs, add);
        ui.separator();
        menu_item_if(msgs, ui, t.tr("sidebar.worktrees.prune"), prune_blocked(snap), Msg::Worktree(WorktreeMsg::Prune));
    };
    let title = t.tr("sidebar.worktrees.header");
    let h = Header { key: "worktrees", title, count: Some(list.len()), add: Some(HeaderAdd::send(add)), menu: Some(&menu), t };
    section_header::show(ui, h, open, msgs);
    if !*open {
        return;
    }
    if list.is_empty() && !filtering {
        section_header::empty_row(ui, t.tr("sidebar.worktrees.empty"), add, msgs);
    }
    let matches = |w: &WorktreeInfo| {
        shown(&w.folder_name()) || w.branch.as_deref().is_some_and(shown) || shown(&trellis_core::native_path::shown(&w.path))
    };
    for wt in list.iter().filter(|w| matches(w)) {
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
        let parts = [widgets::Part::Text(&right)];
        let (r, _, cut) = widgets::list_row_cut(ui, widgets::ROW_INDENT, &label, Some((&parts[..], col)), 0.0, selected, wt.current);
        if r.hovered() {
            *hover_next = wt.head;
        }
        let tip = match open_elsewhere(wt, open_tabs) {
            Some(line) => format!(
                "{}
{line}",
                tooltip(wt)
            ),
            None => tooltip(wt),
        };
        let r = r.on_hover_text(widgets::truncate::full_tip(cut, &label, Some(tip)).unwrap_or_default());
        if r.double_clicked() {
            if open_blocked(wt).is_none() {
                msgs.push(Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
            }
        } else if r.clicked()
            && let Some(o) = wt.head
        {
            msgs.push(Msg::Repo(RepoMsg::JumpTo(o)));
        }
        r.context_menu(|ui| row_menu(ui, wt, msgs));
    }
}

/// The menu of a worktree's row, in the WORKTREES section and on its WIP row in the graph.
pub(super) fn row_menu(ui: &mut egui::Ui, wt: &WorktreeInfo, msgs: &mut Vec<Msg>) {
    menu_item_if(msgs, ui, "Open in new tab", open_blocked(wt), Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
    let gone = wt.missing.then_some("Its folder is missing.");
    menu_item_if(msgs, ui, "Open folder", gone, Msg::OpenFolder(wt.path.clone()));
    menu_item(msgs, ui, "Copy path", Msg::Copy(trellis_core::native_path::shown(&wt.path)));
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
    menu_item_if(msgs, ui, "Move…", move_refused(wt), Msg::Worktree(WorktreeMsg::MoveDialog(path.clone())));
    menu_item_if(msgs, ui, "Remove…", remove_blocked(wt), Msg::Worktree(WorktreeMsg::RemoveDialog(path)));
    if wt.missing {
        menu_item_if(msgs, ui, "Prune missing worktrees", row_prune_blocked(wt), Msg::Worktree(WorktreeMsg::Prune));
    }
}
