//! Selection input on the graph: what a click with modifiers means (on a commit row or the WIP
//! row), the graph's keyboard focus target, and its keys: the arrows (Shift+arrows extend the
//! commit selection), Page Up/Down, Home/End, H and [ ].

use super::ROW_H;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::state::graph_rows::{DisplayRow, GraphKey, GraphRows};
use crate::state::list_keys::{KeySet, ListKey, ListMove};
use crate::state::{Selection, ViewState};
use crate::ui::{list_focus, pane_focus};
use eframe::egui;
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

/// The graph's keys: plain moves (and Shift+Up/Down) of the list keys and Enter, while the graph
/// has the keyboard focus or nothing has.
const KEYS: KeySet = KeySet { extend: true, toggle: false, open: true, delete: false, across: false };

/// The graph's focus target over `rect` (its rows), with `page` rows in view: a click on a row,
/// or the central view's pane key, gives it the focus; it keeps the arrows from egui's focus
/// moves. Registered before the rows, so their clicks still go to them.
pub(super) fn target(ui: &mut egui::Ui, rect: egui::Rect, name: &str, page: usize, focus: bool) {
    let id = pane_focus::graph_id();
    list_focus::target(ui, id, rect, name, page, true);
    if focus {
        list_focus::request(ui.ctx(), id);
    }
}

/// Gives the graph the keyboard focus (a click on a row).
pub(super) fn focus(ctx: &egui::Context) {
    list_focus::request(ctx, pane_focus::graph_id());
}

/// The graph's keys, while it has the keyboard focus or nothing has: Up/Down select the shown
/// row above or below (skipping other worktrees' rows), Page Up/Down a page away, Home/End the
/// first or last row, Enter the details panel's file list, and H, [ and ]
/// (`ViewState::graph_key`, set by their bindings) HEAD's
/// commit or the nearest commit with a branch or tag; the new row is kept in view.
/// Shift+Up/Down extend the commit selection from its moving end instead, as a Shift-click on
/// that row would, stopping at the WIP row; without a selected commit they act as plain arrows.
pub(super) fn keyboard_nav(
    ui: &egui::Ui,
    sel: &Selection,
    snap: &Snapshot,
    rows: &GraphRows,
    scroll: (f32, f32),
    view: &mut ViewState,
    msgs: &mut Vec<Msg>,
) {
    let ctx = ui.ctx();
    let id = pane_focus::graph_id();
    let focused = ctx.memory(|m| m.focused());
    let graph_key = view.graph_key.take();
    if focused.is_some_and(|f| f != id) || rows.is_empty() {
        return;
    }
    let (offset, height) = scroll;
    let page = ((height / ROW_H).floor() as usize).saturating_sub(1).max(1);
    let keys = if focused == Some(id) { list_focus::take(ctx, id, KEYS) } else { list_focus::take_unfocused(ctx, page, KEYS) };
    let len = rows.len();
    // The commit a Shift move extends from (the selection's moving end), when there is one.
    let anchor = crate::state::selection::extend_from(sel);
    let mut cursor = current_row(sel, rows);
    let (mut extending, mut moved, mut on_end) = (false, false, false);
    for key in keys {
        let (mv, extend) = match key {
            // Enter: the details panel's file list (`update/shortcuts.rs`).
            ListKey::Open => {
                msgs.push(Msg::Shortcut(crate::keymap::KeyAction::OpenDetails));
                continue;
            }
            ListKey::Move(mv) => (mv, false),
            ListKey::Extend(mv @ ListMove::Step { .. }) => (mv, anchor.is_some()),
            _ => continue,
        };
        if extend && !on_end {
            // Extending moves the selection's moving end, not its focus row.
            cursor = anchor.and_then(|c| rows.row_of(c)).or(cursor);
            on_end = true;
        }
        let next = match mv {
            ListMove::Step { down } => rows.step(cursor, if down { 1 } else { -1 }),
            ListMove::Page { down, rows: n } => rows.step(cursor.or(Some(0)), if down { n as i64 } else { -(n as i64) }),
            ListMove::Edge { end } => rows.step(Some(if end { len - 1 } else { 0 }), 0),
        };
        // Extending stops at the WIP row and at either end.
        let Some(next) = next.filter(|&n| !(extend && (rows.at(n) == Some(DisplayRow::Wip) || Some(n) == cursor))) else { continue };
        (cursor, extending, moved) = (Some(next), extend, true);
    }
    let target = match graph_key {
        Some(GraphKey::Head) => snap.head_oid.and_then(|h| rows.row_of(h)),
        Some(GraphKey::Ref { up }) => rows.ref_row(snap, cursor, up),
        None => None,
    };
    if let Some(row) = target {
        (cursor, extending, moved) = (Some(row), false, true);
    }
    let Some(next) = cursor.filter(|_| moved) else { return };
    msgs.push(match rows.at(next) {
        Some(DisplayRow::Commit(i)) => match snap.commits.get(i) {
            Some(c) if extending => Msg::Selection(SelectionMsg::SelectCommitRange(c.oid)),
            Some(c) => Msg::Repo(RepoMsg::SelectCommit(c.oid)),
            None => return,
        },
        Some(DisplayRow::Wip) if !extending => Msg::Repo(RepoMsg::SelectWip),
        Some(DisplayRow::Wip | DisplayRow::Worktree(_)) | None => return,
    });
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
