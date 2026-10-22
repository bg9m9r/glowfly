//! Selection input on the graph: what a click with modifiers means (on a commit row or the WIP
//! row), and arrow-key navigation (Shift+arrows extend the commit selection).

use super::ROW_H;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::{Selection, ViewState};
use eframe::egui::{self, Key};
use git2::Oid;
use glowfly_core::repo::Snapshot;

/// The message for a click on commit row `oid`: Shift selects a range (it wins over Ctrl),
/// Ctrl (Cmd on macOS) toggles the commit, and a plain click selects only it.
pub(super) fn row_click(oid: Oid, m: egui::Modifiers) -> Msg {
    if m.shift {
        Msg::Selection(SelectionMsg::SelectCommitRange(oid))
    } else if m.command {
        Msg::Selection(SelectionMsg::ToggleCommit(oid))
    } else {
        Msg::Repo(RepoMsg::SelectCommit(oid))
    }
}

/// The message for a click on the WIP row: Ctrl (Cmd on macOS) without Shift compares the
/// selected commit with the working directory (or stops comparing); anything else selects it.
pub(super) fn wip_click(m: egui::Modifiers) -> Msg {
    if m.command && !m.shift { Msg::Selection(SelectionMsg::ToggleWip) } else { Msg::Repo(RepoMsg::SelectWip) }
}

/// The commit the selection is anchored on: the selected commit, the commit compared with the
/// working directory, or a multi-selection's focus.
pub(super) fn anchor(sel: &Selection) -> Option<Oid> {
    match sel {
        Selection::Commit(o) => Some(*o),
        Selection::VsWorkdir(w) => Some(w.oid),
        Selection::Commits(m) => Some(m.focus()),
        Selection::Wip | Selection::None => None,
    }
}

/// The graph row (counting the WIP row) that keyboard navigation starts from.
pub(super) fn current_row(sel: &Selection, rows: &GraphRows) -> Option<usize> {
    match sel {
        Selection::Wip => rows.has_wip().then_some(0),
        Selection::Commit(o) => rows.row_of(*o),
        Selection::Commits(m) => rows.row_of(m.focus()),
        Selection::VsWorkdir(w) => rows.row_of(w.oid),
        Selection::None => None,
    }
}

/// Arrow Up/Down: select the shown row above or below (skipping other worktrees' rows) and keep
/// it in view. Shift+Up/Down extend the commit selection from its moving end instead, as a
/// Shift-click on that row would, stopping at the WIP row; without a selected commit they act
/// as plain arrows.
pub(super) fn keyboard_nav(ui: &egui::Ui, sel: &Selection, snap: &Snapshot, rows: &GraphRows, view: &mut ViewState, msgs: &mut Vec<Msg>) {
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if typing || rows.is_empty() {
        return;
    }
    let (step, mods) = ui.input(|i| {
        let step = if i.key_pressed(Key::ArrowDown) {
            1i64
        } else if i.key_pressed(Key::ArrowUp) {
            -1
        } else {
            0
        };
        (step, i.modifiers)
    });
    if step == 0 {
        return;
    }
    let extend = (mods.shift && !mods.command && !mods.alt).then(|| crate::state::selection::extend_from(sel)).flatten();
    let from = extend.and_then(|cursor| rows.row_of(cursor)).or_else(|| current_row(sel, rows));
    // Other worktrees' WIP rows are skipped: they can't be selected.
    let Some(next) = rows.step(from, step) else { return };
    if extend.is_some() && Some(next) == from {
        return; // at either end
    }
    msgs.push(match (rows.at(next), extend) {
        (Some(DisplayRow::Commit(i)), _) => match snap.commits.get(i) {
            Some(c) if extend.is_some() => Msg::Selection(SelectionMsg::SelectCommitRange(c.oid)),
            Some(c) => Msg::Repo(RepoMsg::SelectCommit(c.oid)),
            None => return,
        },
        // Extending stops at the WIP row.
        (Some(DisplayRow::Wip), Some(_)) => return,
        (Some(DisplayRow::Wip), None) => Msg::Repo(RepoMsg::SelectWip),
        (Some(DisplayRow::Worktree(_)) | None, _) => return,
    });
    let (offset, height) = view.graph_scroll;
    let top = next as f32 * ROW_H;
    if top < offset {
        view.scroll_offset = Some(top);
    } else if top + ROW_H > offset + height {
        view.scroll_offset = Some(top + ROW_H - height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Modifiers;

    #[test]
    fn wip_click_maps_modifiers() {
        assert!(matches!(wip_click(Modifiers::NONE), Msg::Repo(RepoMsg::SelectWip)));
        assert!(matches!(wip_click(Modifiers::SHIFT), Msg::Repo(RepoMsg::SelectWip)));
        assert!(matches!(wip_click(Modifiers::COMMAND), Msg::Selection(SelectionMsg::ToggleWip)));
        assert!(matches!(wip_click(Modifiers::SHIFT | Modifiers::COMMAND), Msg::Repo(RepoMsg::SelectWip)));
    }

    #[test]
    fn row_click_maps_modifiers() {
        let oid = Oid::from_str("1234567890123456789012345678901234567890").unwrap();
        assert!(matches!(row_click(oid, Modifiers::NONE), Msg::Repo(RepoMsg::SelectCommit(o)) if o == oid));
        assert!(matches!(row_click(oid, Modifiers::COMMAND), Msg::Selection(SelectionMsg::ToggleCommit(o)) if o == oid));
        assert!(matches!(row_click(oid, Modifiers::SHIFT), Msg::Selection(SelectionMsg::SelectCommitRange(o)) if o == oid));
        assert!(
            matches!(row_click(oid, Modifiers::SHIFT | Modifiers::COMMAND), Msg::Selection(SelectionMsg::SelectCommitRange(o)) if o == oid)
        );
    }
}
