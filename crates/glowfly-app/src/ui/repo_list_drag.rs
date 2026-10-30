//! Dragging the welcome screen's favorites by a row's grip: into another order, into a group,
//! from one group to another, or out of the groups into Favorites. While a drag runs an accent
//! line marks the gap the favorite would drop into (the nearest gap of any section; an empty
//! section's gap is its placeholder line), and the release pushes `GroupMsg::MoveRepo`. Only
//! the drag's start is kept between frames (egui's temporary memory); the lists change only
//! when the message is handled.

use super::theme::color;
use super::widgets::{Icon, paint_icon};
use crate::msg::{GroupMsg, Msg};
use eframe::egui::{CursorIcon, Id, PointerButton, Rect, Response, Sense, Stroke, Ui, WidgetInfo, WidgetType, vec2};
use std::path::PathBuf;

/// One section's rows as drawn: the group (None: the ungrouped Favorites), and each favorite's
/// path, grip and row rect in order; with no rows, the placeholder line's rect.
pub(super) struct DropSection {
    pub group: Option<usize>,
    pub rows: Vec<(PathBuf, Response, Rect)>,
    pub empty: Option<Rect>,
}

/// The favorite being dragged: its section (in the list `track` was given) and index there,
/// from the frame the drag started.
#[derive(Clone, Copy)]
struct FavDrag {
    section: usize,
    from: usize,
}

/// The grip at the start of a favorite row (`label` is its accessible name): Grab cursor on
/// hover, Grabbing while dragged.
pub(super) fn grip(ui: &mut Ui, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::drag());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, label));
    let active = resp.hovered() || resp.dragged();
    paint_icon(ui.painter(), rect.center(), Icon::Grip, if active { color::text() } else { color::faint() });
    if resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
    resp
}

/// Tracks a drag among the `sections`: paints the drop line while one runs and pushes the move
/// when it ends somewhere else.
pub(super) fn track(ui: &Ui, sections: &[DropSection], msgs: &mut Vec<Msg>) {
    let id = Id::new("favorite-drag");
    for (section, s) in sections.iter().enumerate() {
        if let Some(from) = s.rows.iter().position(|(_, g, _)| g.drag_started_by(PointerButton::Primary)) {
            ui.data_mut(|d| d.insert_temp(id, FavDrag { section, from }));
        }
    }
    let Some(state) = ui.data(|d| d.get_temp::<FavDrag>(id)) else { return };
    let row = sections.get(state.section).and_then(|s| s.rows.get(state.from)).filter(|(_, g, _)| g.dragged() || g.drag_stopped());
    let (Some((path, grip, _)), Some(pointer)) = (row, ui.input(|i| i.pointer.latest_pos())) else {
        // The drag ended without reaching this row (the list changed under it, say).
        ui.data_mut(|d| d.remove::<FavDrag>(id));
        return;
    };
    let gaps = gaps(sections);
    let Some(&(section, gap, y)) = gaps.iter().min_by(|a, b| (a.2 - pointer.y).abs().total_cmp(&(b.2 - pointer.y).abs())) else {
        return;
    };
    let to = if section == state.section { target(state.from, gap) } else { gap };
    let stays = section == state.section && to == state.from;
    if grip.drag_stopped() {
        ui.data_mut(|d| d.remove::<FavDrag>(id));
        if !stays {
            let group = sections[section].group;
            msgs.push(Msg::Groups(GroupMsg::MoveRepo { path: path.clone(), group, index: to }));
        }
        return;
    }
    if !stays && let Some(x) = sections.iter().flat_map(|s| s.rows.iter().map(|r| r.2).chain(s.empty)).next().map(|r| r.x_range()) {
        ui.painter().hline(x, y, Stroke::new(2.0_f32, color::accent()));
    }
}

/// Every gap a favorite can drop into, as (section, gap, y): above each row and below the
/// last of a section, or an empty section's placeholder.
fn gaps(sections: &[DropSection]) -> Vec<(usize, usize, f32)> {
    let mut out = Vec::new();
    for (k, s) in sections.iter().enumerate() {
        let rects: Vec<Rect> = s.rows.iter().map(|r| r.2).collect();
        match (rects.first(), rects.last()) {
            (Some(first), Some(last)) => {
                out.push((k, 0, first.top()));
                for g in 1..rects.len() {
                    out.push((k, g, (rects[g - 1].bottom() + rects[g].top()) / 2.0));
                }
                out.push((k, rects.len(), last.bottom()));
            }
            _ => out.extend(s.empty.map(|r| (k, 0, r.center().y))),
        }
    }
    out
}

/// Where the favorite at `from` ends up when dropped into gap `gap` of its own section (0 is
/// above the first row, `n` below the last).
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
