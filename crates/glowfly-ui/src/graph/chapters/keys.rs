//! The Chapters list's keys, while it has the keyboard focus or nothing has: Up/Down, Page
//! Up/Down and Home/End select the row they land on (headings are skipped) and keep it in view,
//! Right expands the current chapter, Left collapses it (or goes from a commit to its chapter),
//! and Enter opens the details panel's file list.

use crate::{list_focus, pane_focus};
use eframe::egui;
use glowfly_core::repo::Snapshot;
use glowfly_model::keymap::KeyAction;
use glowfly_model::msg::{ChaptersMsg, Msg, RepoMsg};
use glowfly_model::state::chapters::{Row, moves};
use glowfly_model::state::list_keys::{KeySet, ListKey};
use glowfly_model::state::{AppState, ViewState};

/// The plain moves, Enter, and Left and Right.
const KEYS: KeySet = KeySet { extend: false, toggle: false, open: true, delete: false, across: true };

/// Handles this frame's keys; the scroll offset that brings the row a move landed on into view,
/// when it isn't. `wip` says whether the work tree's row leads the list.
pub(super) fn keyboard(
    ui: &egui::Ui,
    st: &AppState,
    snap: &Snapshot,
    wip: bool,
    (offset, height, row_h): (f32, f32, f32),
    view: &mut ViewState,
    msgs: &mut Vec<Msg>,
) -> Option<f32> {
    let ctx = ui.ctx();
    let id = pane_focus::graph_id();
    let focused = ctx.memory(|m| m.focused());
    // H, [ and ] are the graph's.
    view.graph_key.take();
    let cv = &st.session.chapters;
    let list = cv.list.as_ref()?;
    if focused.is_some_and(|f| f != id) || cv.rows.is_empty() && !wip {
        return None;
    }
    let page = ((height / row_h).floor() as usize).saturating_sub(1).max(1);
    let keys = if focused == Some(id) { list_focus::take(ctx, id, KEYS) } else { list_focus::take_unfocused(ctx, page, KEYS) };
    let base = usize::from(wip);
    let row_at = |d: usize| d.checked_sub(base).and_then(|i| cv.rows.get(i)).copied();
    let mut cursor = moves::current(cv, snap, &st.session.selection, st.session.wip_lit, wip);
    let mut moved = false;
    for key in keys {
        match key {
            ListKey::Open => msgs.push(Msg::Shortcut(KeyAction::OpenDetails)),
            ListKey::Move(mv) => {
                if let Some(next) = moves::step(&cv.rows, wip, cursor, mv) {
                    (cursor, moved) = (Some(next), true);
                }
            }
            ListKey::Across { right } => match cursor.and_then(row_at) {
                Some(Row::Chapter(c)) => {
                    let Some(ch) = list.list.get(c as usize) else { continue };
                    let (expanded, expandable) = (cv.expanded.contains(&ch.id), !ch.inside().is_empty());
                    if (right && !expanded && expandable) || (!right && expanded) {
                        msgs.push(Msg::Chapters(ChaptersMsg::Toggle(ch.id)));
                    }
                }
                Some(Row::Commit { .. }) if !right => {
                    if let Some(parent) = cursor.and_then(|d| moves::parent_row(&cv.rows, wip, d)) {
                        (cursor, moved) = (Some(parent), true);
                    }
                }
                _ => {}
            },
            ListKey::Extend(_) | ListKey::Toggle | ListKey::Delete => {}
        }
    }
    let next = cursor.filter(|_| moved)?;
    let msg = match row_at(next) {
        None if wip && next == 0 => Msg::Repo(RepoMsg::SelectWip),
        Some(Row::Chapter(c)) => Msg::Chapters(ChaptersMsg::Select(list.list.get(c as usize)?.id)),
        Some(Row::Commit { row, .. }) => Msg::Repo(RepoMsg::SelectCommit(snap.commits.get(row as usize)?.oid)),
        Some(Row::Release(_)) | None => return None,
    };
    msgs.push(msg);
    let top = next as f32 * row_h;
    if top < offset {
        Some(top)
    } else if top + row_h > offset + height {
        Some(top + row_h - height)
    } else {
        None
    }
}
