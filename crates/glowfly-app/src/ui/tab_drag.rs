//! Dragging a repository tab sideways to reorder it (`tabs.rs` draws the strip): the drag is
//! kept in egui's temporary memory from its start to the drop, Esc snaps the tab back, and the
//! drop pushes `MoveTab` when it changes the order. Tabs are as wide as their names, so the slot
//! a drop lands on is the one whose centre is nearest the floating tab's, and the other tabs
//! slide (animated) to where the drop would put them (`lefts_after`). In a drawn title bar the
//! only tab has nothing to swap with, so dragging it moves the window, as the empty bar does.

use crate::msg::{AppMsg, Msg, TabsMsg};
use crate::state::tab_groups::{is_identity, move_order};
use crate::state::window_chrome::WindowCmd;
use crate::ui::theme::tokens::motion;
use eframe::egui::{self, CursorIcon, Id, Key, PointerButton, Rect, Response, pos2};
use std::ops::Range;

/// Where the drag in progress is kept in egui's temporary memory.
const DRAG_ID: &str = "tab-drag";
/// How long a tab takes to slide out of the dragged tab's way.
pub(super) const SLIDE: f32 = motion::STANDARD;

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
/// tabs that move together, `tab_groups::move_order`). With `moves_window` (a drawn title bar),
/// dragging the only tab moves the window instead.
pub(super) fn drag(ui: &egui::Ui, slots: &[Slot], blocks: &[Range<usize>], moves_window: bool, msgs: &mut Vec<Msg>) -> Option<Floating> {
    let drag_id = Id::new(DRAG_ID);
    let n = slots.len();
    if n < 2 {
        ui.data_mut(|d| d.remove::<TabDrag>(drag_id));
        if moves_window && slots.first().is_some_and(|s| s.r.drag_started_by(PointerButton::Primary)) {
            msgs.push(Msg::App(AppMsg::Window(WindowCmd::StartDrag)));
        }
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
    let (first, last) = (slots[0].rect.left(), slots[n - 1].rect.right());
    let w = slot.rect.width();
    let left = (px.x - state.grab).clamp(first, (last - w).max(first));
    let spans: Vec<(f32, f32)> = slots.iter().map(|s| (s.rect.left(), s.rect.width())).collect();
    let to = drop_slot(left + w / 2.0, &spans);
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

/// The slot (each a left edge and a width) whose centre is nearest `centre`.
fn drop_slot(centre: f32, spans: &[(f32, f32)]) -> usize {
    let off = |&(left, w): &(f32, f32)| (left + w / 2.0 - centre).abs();
    (0..spans.len()).min_by(|&a, &b| off(&spans[a]).total_cmp(&off(&spans[b]))).unwrap_or(0)
}

/// Where each tab's left edge goes in `order` (`order[new] == old`), the tabs being `widths`
/// wide and `gap` apart from `start`: indexed by the tab's old index.
pub(super) fn lefts_after(order: &[usize], widths: &[f32], start: f32, gap: f32) -> Vec<f32> {
    let mut lefts = vec![start; widths.len()];
    let mut x = start;
    for &old in order {
        lefts[old] = x;
        x += widths[old] + gap;
    }
    lefts
}

#[cfg(test)]
mod tests {
    use super::{drop_slot, lefts_after};

    #[test]
    fn drop_slot_takes_the_nearest_centre() {
        // Three tabs 100, 200 and 120 wide, 4 apart from 6: centres at 56, 210 and 374.
        let spans = [(6.0, 100.0), (110.0, 200.0), (314.0, 120.0)];
        assert_eq!(drop_slot(-50.0, &spans), 0);
        assert_eq!(drop_slot(56.0, &spans), 0);
        assert_eq!(drop_slot(132.0, &spans), 0);
        assert_eq!(drop_slot(134.0, &spans), 1);
        assert_eq!(drop_slot(291.0, &spans), 1);
        assert_eq!(drop_slot(293.0, &spans), 2);
        assert_eq!(drop_slot(5000.0, &spans), 2);
    }

    #[test]
    fn the_others_slide_by_the_dragged_tabs_width() {
        let widths = [100.0, 200.0, 120.0];
        assert_eq!(lefts_after(&[0, 1, 2], &widths, 6.0, 4.0), [6.0, 110.0, 314.0]);
        // Tab 0 dropped on slot 1: tab 1 moves left by 100 + 4, tab 2 stays.
        assert_eq!(lefts_after(&[1, 0, 2], &widths, 6.0, 4.0), [210.0, 6.0, 314.0]);
        // Tab 2 dropped on slot 0: tabs 0 and 1 move right by 120 + 4.
        assert_eq!(lefts_after(&[2, 0, 1], &widths, 6.0, 4.0), [130.0, 234.0, 6.0]);
    }
}
