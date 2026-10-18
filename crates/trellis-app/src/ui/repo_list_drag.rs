//! Reordering the welcome screen's favorites by dragging a row's grip: while a drag runs an
//! accent line marks the gap the favorite would drop into, and the release pushes
//! `RecentMsg::MoveFavorite`. Only the drag's start is kept between frames (egui's temporary
//! memory); the favorites change only when the message is handled.

use super::theme::color;
use super::widgets::{Icon, paint_icon};
use crate::msg::{Msg, RecentMsg};
use eframe::egui::{CursorIcon, Id, PointerButton, Rect, Response, Sense, Stroke, Ui, WidgetInfo, WidgetType, vec2};

/// The favorite being dragged (its index), from the frame the drag started.
#[derive(Clone, Copy)]
struct FavDrag {
    from: usize,
}

const GRIP_LABEL: &str = "Drag to reorder";

/// The grip at the start of a favorite row: Grab cursor on hover, Grabbing while dragged.
pub(super) fn grip(ui: &mut Ui) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::drag());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, GRIP_LABEL));
    let active = resp.hovered() || resp.dragged();
    paint_icon(ui.painter(), rect.center(), Icon::Grip, if active { color::text() } else { color::faint() });
    if resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
    resp
}

/// Tracks a drag among the favorite `rows` (each grip's response and its row's rect, in
/// order): paints the drop line while one runs and pushes the move when it ends somewhere else.
pub(super) fn track(ui: &Ui, rows: &[(Response, Rect)], msgs: &mut Vec<Msg>) {
    let id = Id::new("favorite-drag");
    if let Some(from) = rows.iter().position(|(g, _)| g.drag_started_by(PointerButton::Primary)) {
        ui.data_mut(|d| d.insert_temp(id, FavDrag { from }));
    }
    let Some(state) = ui.data(|d| d.get_temp::<FavDrag>(id)) else { return };
    let grip = rows.get(state.from).map(|(g, _)| g).filter(|g| g.dragged() || g.drag_stopped());
    let (Some(grip), Some(pointer)) = (grip, ui.input(|i| i.pointer.latest_pos())) else {
        // The drag ended without reaching this row (the list changed under it, say).
        ui.data_mut(|d| d.remove::<FavDrag>(id));
        return;
    };
    let gap = rows.iter().filter(|(_, r)| r.center().y < pointer.y).count();
    let to = target(state.from, gap);
    if grip.drag_stopped() {
        ui.data_mut(|d| d.remove::<FavDrag>(id));
        if to != state.from {
            msgs.push(Msg::Recent(RecentMsg::MoveFavorite { from: state.from, to }));
        }
        return;
    }
    if to != state.from {
        let y = match gap {
            0 => rows[0].1.top(),
            g if g == rows.len() => rows[g - 1].1.bottom(),
            g => (rows[g - 1].1.bottom() + rows[g].1.top()) / 2.0,
        };
        ui.painter().hline(rows[0].1.x_range(), y, Stroke::new(2.0_f32, color::accent()));
    }
}

/// Where the favorite at `from` ends up when dropped into gap `gap` (0 is above the first row,
/// `n` below the last).
fn target(from: usize, gap: usize) -> usize {
    if gap > from { gap - 1 } else { gap }
}

#[cfg(test)]
mod tests {
    use super::target;

    #[test]
    fn gaps_become_final_indexes() {
        assert_eq!(target(1, 0), 0, "above the first");
        assert_eq!(target(0, 3), 2, "below the last of three");
        assert_eq!(target(1, 1), 1, "its own gaps leave it where it is");
        assert_eq!(target(1, 2), 1);
        assert_eq!(target(2, 1), 1);
    }
}
