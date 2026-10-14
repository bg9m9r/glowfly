//! Selection input on the graph: what a click with modifiers means, and arrow-key navigation.

use super::ROW_H;
use crate::msg::Msg;
use crate::state::{Selection, ViewState};
use eframe::egui::{self, Key};
use git2::Oid;
use trellis_core::repo::Snapshot;

/// The message for a click on commit row `oid`: Shift selects a range (it wins over Ctrl),
/// Ctrl (Cmd on macOS) toggles the commit, and a plain click selects only it.
pub(super) fn row_click(oid: Oid, m: egui::Modifiers) -> Msg {
    if m.shift {
        Msg::SelectCommitRange(oid)
    } else if m.command {
        Msg::ToggleCommit(oid)
    } else {
        Msg::SelectCommit(oid)
    }
}

/// The graph row (counting the WIP row) that keyboard navigation starts from.
pub(super) fn current_row(sel: &Selection, snap: &Snapshot, has_wip: bool) -> Option<usize> {
    let commit_row = |o: &Oid| snap.row_of.get(o).map(|r| r + has_wip as usize);
    match sel {
        Selection::Wip => Some(0),
        Selection::Commit(o) => commit_row(o),
        Selection::Commits(m) => commit_row(&m.focus()),
        Selection::None => None,
    }
}

/// Arrow Up/Down: select the row above or below and keep it in view.
pub(super) fn keyboard_nav(
    ui: &egui::Ui,
    sel: &Selection,
    snap: &Snapshot,
    view: &mut ViewState,
    total: usize,
    has_wip: bool,
    msgs: &mut Vec<Msg>,
) {
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if typing {
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
    let current = current_row(sel, snap, has_wip);
    let next = current.map_or(0, |c| (c as i64 + step).clamp(0, total as i64 - 1) as usize);
    msgs.push(if has_wip && next == 0 { Msg::SelectWip } else { Msg::SelectCommit(snap.commits[next - has_wip as usize].oid) });
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
        assert!(matches!(row_click(oid, Modifiers::NONE), Msg::SelectCommit(o) if o == oid));
        assert!(matches!(row_click(oid, Modifiers::COMMAND), Msg::ToggleCommit(o) if o == oid));
        assert!(matches!(row_click(oid, Modifiers::SHIFT), Msg::SelectCommitRange(o) if o == oid));
        assert!(matches!(row_click(oid, Modifiers::SHIFT | Modifiers::COMMAND), Msg::SelectCommitRange(o) if o == oid));
    }
}
