//! Dragging rebase panel rows to reorder them: the grab, the slot a row would drop into, and
//! the `RebaseMove` sent on release.

use super::rows::{GRIP_X, ROW_H, Slot};
use crate::msg::{Msg, RebaseMsg};
use eframe::egui::{CursorIcon, Id, PointerButton, Rect, Ui, pos2, vec2};

/// A row being dragged: its index and where on it the pointer grabbed it (from its top edge).
/// Kept in egui's temporary memory, since the press position is gone by the release frame.
#[derive(Clone, Copy)]
struct RowDrag {
    from: usize,
    grab: f32,
}

/// A drag in progress: the dragged row, the slot it would drop into, and where it is drawn.
#[derive(Clone, Copy)]
pub(super) struct Floating {
    pub from: usize,
    pub to: usize,
    pub rect: Rect,
}

/// Tracks a row drag: returns where the dragged row floats while one is in progress, and
/// pushes `RebaseMove` when it is dropped in another slot.
pub(super) fn drag(ui: &Ui, slots: &[Slot], list_top: f32, pitch: f32, msgs: &mut Vec<Msg>) -> Option<Floating> {
    let drag_id = Id::new("rebase-drag");
    let n = slots.len();
    if let Some((i, s)) = slots.iter().enumerate().find(|(_, s)| s.movable && s.r.drag_started_by(PointerButton::Primary))
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        ui.data_mut(|d| d.insert_temp(drag_id, RowDrag { from: i, grab: origin.y - s.rect.top() }));
    }
    let state = ui.data(|d| d.get_temp::<RowDrag>(drag_id))?;
    let slot = slots.get(state.from).filter(|s| s.r.dragged() || s.r.drag_stopped());
    let (Some(slot), Some(p)) = (slot, ui.input(|i| i.pointer.latest_pos())) else {
        // The drag ended without reaching this row (e.g. the list changed under it).
        ui.data_mut(|d| d.remove::<RowDrag>(drag_id));
        return None;
    };
    let top = (p.y - state.grab).clamp(list_top, list_top + (n - 1) as f32 * pitch);
    let to = drop_slot(top, list_top, pitch, n);
    if slot.r.drag_stopped() {
        ui.data_mut(|d| d.remove::<RowDrag>(drag_id));
        if to != state.from {
            msgs.push(Msg::Rebase(RebaseMsg::RebaseMove { from: state.from, to }));
        }
        return None;
    }
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    Some(Floating { from: state.from, to, rect: Rect::from_min_size(pos2(slot.rect.left(), top), slot.rect.size()) })
}

/// The slot nearest to a row whose top edge is at `top`.
fn drop_slot(top: f32, list_top: f32, pitch: f32, n: usize) -> usize {
    (((top - list_top) / pitch).round().max(0.0) as usize).min(n - 1)
}

pub(super) fn grip_rect(row: Rect) -> Rect {
    Rect::from_center_size(pos2(row.left() + GRIP_X, row.center().y), vec2(20.0, ROW_H))
}

#[cfg(test)]
mod tests {
    use super::drop_slot;

    #[test]
    fn drop_slot_rounds_and_clamps() {
        assert_eq!(drop_slot(-50.0, 100.0, 42.0, 4), 0);
        assert_eq!(drop_slot(100.0 + 20.0, 100.0, 42.0, 4), 0);
        assert_eq!(drop_slot(100.0 + 22.0, 100.0, 42.0, 4), 1);
        assert_eq!(drop_slot(100.0 + 84.0, 100.0, 42.0, 4), 2);
        assert_eq!(drop_slot(5000.0, 100.0, 42.0, 4), 3);
    }
}
