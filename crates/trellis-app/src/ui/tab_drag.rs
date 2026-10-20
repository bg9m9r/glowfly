//! Dragging a repository tab sideways to reorder it (`tabs.rs` draws the strip): the drag is
//! kept in egui's temporary memory from its start to the drop, Esc snaps the tab back, and the
//! drop pushes `MoveTab` when it changes the order.

use crate::msg::{Msg, TabsMsg};
use crate::state::tab_groups::{is_identity, move_order};
use eframe::egui::{self, CursorIcon, Id, Key, PointerButton, Rect, Response, pos2};
use std::ops::Range;

/// Where the drag in progress is kept in egui's temporary memory.
const DRAG_ID: &str = "tab-drag";

/// A tab being dragged: its index and where on it the pointer grabbed it (from its left edge).
/// Kept in egui's temporary memory, since the press position is gone by the release frame.
/// `cancelled` once Esc snapped it back: the rest of that drag does nothing.
#[derive(Clone, Copy)]
struct TabDrag {
    from: usize,
    grab: f32,
    cancelled: bool,
}

/// Whether a tab is being dragged now (and the drag wasn't cancelled): Esc then cancels the
/// drag and nothing else.
pub(crate) fn dragging(ctx: &egui::Context) -> bool {
    let drag = ctx.data(|d| d.get_temp::<TabDrag>(Id::new(DRAG_ID)));
    drag.is_some_and(|d| !d.cancelled) && ctx.input(|i| i.pointer.primary_down())
}

/// A drag in progress: the dragged tab, the order a drop now would give (`order[new] == old`),
/// and where it is drawn.
pub(super) struct Floating {
    pub from: usize,
    pub order: Vec<usize>,
    pub rect: Rect,
}

/// One tab's place in the strip and its interactions this frame.
pub(super) struct Slot {
    pub rect: Rect,
    pub r: Response,
    pub close: Response,
}

/// Tracks a tab drag: returns where the dragged tab floats while one is in progress, and pushes
/// `MoveTab` with the slot under it when dropping it there changes the order (`blocks` are the
/// tabs that move together, `tab_groups::move_order`).
pub(super) fn drag(
    ui: &egui::Ui,
    slots: &[Slot],
    blocks: &[Range<usize>],
    strip_left: f32,
    pitch: f32,
    msgs: &mut Vec<Msg>,
) -> Option<Floating> {
    let drag_id = Id::new(DRAG_ID);
    let n = slots.len();
    if n < 2 {
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        return None;
    }
    if let Some((i, s)) = slots.iter().enumerate().find(|(_, s)| s.r.drag_started_by(PointerButton::Primary))
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        ui.data_mut(|d| d.insert_temp(drag_id, TabDrag { from: i, grab: origin.x - s.rect.left(), cancelled: false }));
    }
    let mut state = ui.data(|d| d.get_temp::<TabDrag>(drag_id))?;
    let slot = slots.get(state.from).filter(|s| s.r.dragged() || s.r.drag_stopped());
    let (Some(slot), Some(px)) = (slot, ui.input(|i| i.pointer.latest_pos())) else {
        // The drag ended without reaching this tab (e.g. the tabs changed under it).
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        return None;
    };
    if !state.cancelled && ui.input(|i| i.key_pressed(Key::Escape)) {
        state.cancelled = true;
        ui.data_mut(|d| d.insert_temp(drag_id, state));
    }
    if state.cancelled {
        // Snapped back: the tab stays in its slot until the button is released.
        if slot.r.drag_stopped() {
            ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        }
        return None;
    }
    let left = (px.x - state.grab).clamp(strip_left, strip_left + (n - 1) as f32 * pitch);
    let to = drop_slot(left, strip_left, pitch, n);
    let order = move_order(blocks, state.from, to);
    if slot.r.drag_stopped() {
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        if !is_identity(&order) {
            msgs.push(Msg::Tabs(TabsMsg::MoveTab { from: state.from, to }));
        }
        return None;
    }
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    Some(Floating { from: state.from, order, rect: Rect::from_min_size(pos2(left, slot.rect.top()), slot.rect.size()) })
}

/// The slot nearest to a tab whose left edge is at `left`.
fn drop_slot(left: f32, strip_left: f32, pitch: f32, n: usize) -> usize {
    (((left - strip_left) / pitch).round().max(0.0) as usize).min(n - 1)
}

#[cfg(test)]
mod tests {
    use super::drop_slot;

    #[test]
    fn drop_slot_rounds_and_clamps() {
        assert_eq!(drop_slot(-50.0, 6.0, 221.0, 3), 0);
        assert_eq!(drop_slot(6.0, 6.0, 221.0, 3), 0);
        assert_eq!(drop_slot(6.0 + 110.0, 6.0, 221.0, 3), 0);
        assert_eq!(drop_slot(6.0 + 111.0, 6.0, 221.0, 3), 1);
        assert_eq!(drop_slot(6.0 + 442.0, 6.0, 221.0, 3), 2);
        assert_eq!(drop_slot(5000.0, 6.0, 221.0, 3), 2);
    }
}
