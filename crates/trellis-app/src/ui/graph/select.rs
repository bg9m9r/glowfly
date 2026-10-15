//! Selection input on the graph: what a click with modifiers means, and arrow-key navigation.

use super::ROW_H;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::state::search::GraphRows;
use crate::state::{Selection, ViewState};
use eframe::egui::{self, Key};
use git2::Oid;
use trellis_core::repo::Snapshot;

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

/// The graph row (counting the WIP row) that keyboard navigation starts from.
pub(super) fn current_row(sel: &Selection, rows: &GraphRows) -> Option<usize> {
    match sel {
        Selection::Wip => rows.has_wip().then_some(0),
        Selection::Commit(o) => rows.row_of(*o),
        Selection::Commits(m) => rows.row_of(m.focus()),
        Selection::None => None,
    }
}

/// Arrow Up/Down: select the shown row above or below and keep it in view.
pub(super) fn keyboard_nav(ui: &egui::Ui, sel: &Selection, snap: &Snapshot, rows: &GraphRows, view: &mut ViewState, msgs: &mut Vec<Msg>) {
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if typing || rows.is_empty() {
        return;
    }
    let step = ui.input(|i| {
        if i.key_pressed(Key::ArrowDown) {
            1i64
        } else if i.key_pressed(Key::ArrowUp) {
            -1
        } else {
            0
        }
    });
    if step == 0 {
        return;
    }
    let current = current_row(sel, rows);
    let next = current.map_or(0, |c| (c as i64 + step).clamp(0, rows.len() as i64 - 1) as usize);
    msgs.push(match rows.commit_at(next).and_then(|i| snap.commits.get(i)) {
        Some(c) => Msg::Repo(RepoMsg::SelectCommit(c.oid)),
        None if rows.has_wip() && next == 0 => Msg::Repo(RepoMsg::SelectWip),
        None => return,
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
