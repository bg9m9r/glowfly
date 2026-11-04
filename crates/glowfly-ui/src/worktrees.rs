//! The WORKTREES section of the left panel: once a worktree is linked, one row per worktree (the
//! main one first) with what it has checked out, whether it is locked or missing, and a tooltip;
//! before that, once expanded, a sentence on what worktrees are for and Add worktree…. A click jumps to its HEAD commit, a
//! double-click opens it in a tab, and the row menu (also on the worktree's WIP row in the graph)
//! opens it, shows its folder, copies its path, locks or unlocks it, moves it, removes it, and
//! prunes missing records. The header's + adds a worktree from HEAD, and its menu adds one or
//! prunes missing records.

use super::common::{menu_danger, menu_item, menu_item_if};
use super::theme::{color, type_scale};
use super::widgets;
use super::widgets::empty_state::{self, Action};
use super::widgets::section_header::{self, Header, HeaderAdd, action_item};
use crate::widgets::menu::separator;
use crate::widgets::row_menu::RowMenuExt;
use eframe::egui;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Snapshot, WorktreeInfo};
use glowfly_model::msg::Msg;
use glowfly_model::msg::RepoMsg;
use glowfly_model::state::Selection;
use glowfly_model::state::section_actions::SectionAction;
use glowfly_model::state::worktree_manage::{manage_blocked, open_elsewhere, prune_blocked, remove_blocked, row_prune_blocked};
use glowfly_model::state::worktree_move::move_refused;
use glowfly_model::state::worktrees::{listed, open_blocked, right_text, tooltip};
use glowfly_model::worktree::WorktreeMsg;

/// The share of a worktree row its dim right-hand text (what it has checked out) keeps when the
/// name and it don't both fit.
const RIGHT_SHARE: f32 = 0.4;

/// What the rows are drawn against: the left-panel filter (and whether it has text), the
/// selection, the folders open in tabs (their tooltips say so), and the header's Add worktree.
pub(super) struct Rows<'a> {
    pub shown: &'a dyn Fn(&str) -> bool,
    pub sel: &'a Selection,
    pub open_tabs: &'a [glowfly_model::state::tabs::Place],
    pub add: &'a SectionAction,
    pub filtering: bool,
    pub t: Lang,
}

pub(super) fn section(ui: &mut egui::Ui, snap: &Snapshot, open: &mut bool, rows: Rows, hover_next: &mut Option<Oid>, msgs: &mut Vec<Msg>) {
    let Rows { shown, sel, open_tabs, add, filtering, t } = rows;
    let list = listed(snap);
    let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| {
        action_item(ui, msgs, add);
        separator(ui);
        menu_item_if(msgs, ui, t.tr("sidebar.worktrees.prune"), prune_blocked(snap), Msg::Worktree(WorktreeMsg::Prune));
    };
    let title = t.tr("sidebar.worktrees.header");
    let h = Header { key: "worktrees", title, count: Some(list.len()), folds: true, add: Some(HeaderAdd::send(add)), menu: Some(&menu), t };
    section_header::show(ui, h, open, msgs);
    if !*open {
        return;
    }
    if list.is_empty() && !filtering {
        empty_state::show(ui, t.tr("sidebar.empty.worktrees"), vec![Action::of(add)], msgs);
    }
    let matches = |w: &WorktreeInfo| {
        shown(&w.folder_name()) || w.branch.as_deref().is_some_and(shown) || shown(&glowfly_core::native_path::shown(&w.path))
    };
    for wt in list.iter().filter(|w| matches(w)) {
        let label = if wt.current { format!("✔  {}", wt.folder_name()) } else { wt.folder_name() };
        let col = if wt.locked.is_some() {
            color::warn()
        } else if wt.missing {
            color::removed()
        } else {
            color::weak()
        };
        let selected = wt.head.is_some_and(|o| *sel == Selection::Commit(o));
        let right = right_shown(ui, wt, &label);
        let parts = [widgets::Part::Text(&right)];
        let right = (!right.is_empty()).then_some((&parts[..], col));
        let (r, _, cut) = widgets::name_row_cut(ui, widgets::ROW_INDENT, &label, right, 0.0, selected, wt.current);
        if wt.current {
            crate::access::name_button(&r, true, &t.trf("access.current_worktree", &[("name", &wt.folder_name())]));
        }
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
        r.row_menu(|ui| row_menu(ui, wt, msgs));
    }
}

/// The menu of a worktree's row, in the WORKTREES section and on its WIP row in the graph.
pub(super) fn row_menu(ui: &mut egui::Ui, wt: &WorktreeInfo, msgs: &mut Vec<Msg>) {
    menu_item_if(msgs, ui, "Open in new tab", open_blocked(wt), Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
    let gone = wt.missing.then_some("Its folder is missing.");
    menu_item_if(msgs, ui, "Open folder", gone, Msg::OpenFolder(wt.path.clone()));
    menu_item(msgs, ui, "Copy path", Msg::Copy(glowfly_core::native_path::shown(&wt.path)));
    // git refuses all of these for the main worktree.
    if wt.is_main() {
        return;
    }
    separator(ui);
    let path = wt.path.clone();
    if wt.locked.is_some() {
        menu_item_if(msgs, ui, "Unlock", manage_blocked(wt), Msg::Worktree(WorktreeMsg::Unlock(path.clone())));
    } else {
        menu_item_if(msgs, ui, "Lock…", manage_blocked(wt), Msg::Worktree(WorktreeMsg::LockDialog(path.clone())));
    }
    menu_item_if(msgs, ui, "Move…", move_refused(wt), Msg::Worktree(WorktreeMsg::MoveDialog(path.clone())));
    if wt.missing {
        menu_item_if(msgs, ui, "Prune missing worktrees", row_prune_blocked(wt), Msg::Worktree(WorktreeMsg::Prune));
    }
    separator(ui);
    menu_danger(msgs, ui, "Remove…", remove_blocked(wt), Msg::Worktree(WorktreeMsg::RemoveDialog(path)));
}

/// What worktree `wt`'s row (named `label`) shows at its right: what it has checked out, left
/// out when the branch only repeats the worktree's name, then its status (" · locked"), never
/// cut. When the name and all that don't both fit, it takes what the name leaves, but at least
/// `RIGHT_SHARE` of the row: a branch is cut in its middle, a detached head shows its hash alone,
/// then nothing. The tooltip has it all whole.
fn right_shown(ui: &egui::Ui, wt: &WorktreeInfo, label: &str) -> String {
    let full = right_text(wt);
    let what = full.split(" · ").next().unwrap_or_default();
    let status = &full[what.len()..];
    let what = if wt.branch.as_deref().is_some_and(|b| repeats(b, &wt.folder_name())) { "" } else { what };
    let join = |w: &str| if w.is_empty() { status.trim_start_matches(" · ").to_owned() } else { format!("{w}{status}") };
    let p = ui.painter();
    let small = type_scale::font(type_scale::SMALL);
    let width = |s: &str| p.layout_no_wrap(s.to_owned(), small.clone(), egui::Color32::PLACEHOLDER).size().x;
    let font = if wt.current { type_scale::strong(ui, type_scale::BODY) } else { type_scale::font(type_scale::BODY) };
    let name_w = p.layout_no_wrap(label.to_owned(), font, egui::Color32::PLACEHOLDER).size().x;
    let row = ui.available_width() - widgets::ROW_INDENT - widgets::PAD_X;
    let whole = join(what);
    if name_w + widgets::NAME_GAP + width(&whole) <= row {
        return whole;
    }
    let room = (row - name_w - widgets::NAME_GAP).max(row * RIGHT_SHARE).min(width(&whole)) - width(status);
    let what = if wt.branch.is_some() {
        Some(widgets::truncate::middle(p, what, &small, room)).filter(|w| w != "…").unwrap_or_default()
    } else {
        let hash = what.rsplit(' ').next().unwrap_or_default();
        [what, hash].into_iter().find(|w| width(w) <= room).unwrap_or_default().to_owned()
    };
    join(&what)
}

/// Whether `branch` only repeats worktree folder `folder`: the same, or ending in it after a
/// separator (`worktree-agent-x` for `agent-x`).
fn repeats(branch: &str, folder: &str) -> bool {
    branch.strip_suffix(folder).is_some_and(|rest| rest.is_empty() || rest.ends_with(['/', '-', '_', '.']))
}

#[cfg(test)]
mod tests {
    use super::repeats;

    #[test]
    fn a_branch_that_repeats_the_folder_name_is_left_out() {
        assert!(repeats("agent-a3f9", "agent-a3f9"));
        assert!(repeats("worktree-agent-a3f9", "agent-a3f9"));
        assert!(repeats("feature/login", "login"));
        assert!(!repeats("feature-login", "hotfix"));
        assert!(!repeats("blogin", "login"), "only after a separator");
    }
}
