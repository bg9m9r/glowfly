//! Dragging the graph header's dividers to resize columns, and double-clicking one to reset its
//! column. The drag lives in `ViewState::column_drag` as a preview; only the release sends the
//! width to `update`, which saves it.

use super::paint::Columns;
use crate::msg::{GraphMsg, Msg};
use crate::state::ColumnDrag;
use crate::ui::theme::color;
use eframe::egui::{self, CursorIcon, Rect, Sense, Stroke, WidgetInfo, WidgetType, pos2};
use trellis_core::settings::{ColumnWidths, GraphColumns, ResizableColumn, drag_width};

/// Half the width of a divider's grab area.
const GRIP_HALF_W: f32 = 4.0;

/// Interacts with each shown column's divider: a drag updates `drag` (the preview), its release
/// and a double-click push the message that stores the width. Returns the column whose divider
/// is hovered or dragged, to highlight.
pub(super) fn grips(
    ui: &egui::Ui,
    hdr: Rect,
    cols: &Columns,
    cfg: &GraphColumns,
    saved: &ColumnWidths,
    drag: &mut Option<ColumnDrag>,
    msgs: &mut Vec<Msg>,
) -> Option<ResizableColumn> {
    let mut held = false;
    let mut hovered = None;
    for (col, x) in cols.grips(cfg) {
        let rect = Rect::from_x_y_ranges(x - GRIP_HALF_W..=x + GRIP_HALF_W, hdr.y_range());
        let resp = ui.interact(rect, ui.id().with(("column-grip", col)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::ResizeColumn);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, format!("Resize {} column", col.title())));
        if resp.hovered() {
            hovered = Some(col);
        }
        if resp.double_clicked() {
            *drag = None;
            msgs.push(Msg::Graph(GraphMsg::ResetColumnWidth(col)));
            continue;
        }
        if resp.drag_started() {
            // From where the button went down, not where egui decided it was a drag: no jump.
            let start_x = ui.input(|i| i.pointer.press_origin()).map_or(x, |p| p.x);
            let w = saved.get(col);
            *drag = Some(ColumnDrag { col, start_x, start_width: w, room: cols.msg_room(), width: w });
        }
        if resp.dragged() {
            held = true;
            if let Some(d) = drag.as_mut().filter(|d| d.col == col) {
                if let Some(p) = resp.interact_pointer_pos() {
                    d.width = drag_width(col, d.start_width, p.x - d.start_x, d.room);
                }
                ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
            }
        }
        if resp.drag_stopped() {
            held = true;
            if let Some(d) = drag.take_if(|d| d.col == col) {
                msgs.push(Msg::Graph(GraphMsg::ResizeColumn { col, width: d.width }));
            }
        }
    }
    // A release this view never saw (a tab switch, the pointer leaving the window): no preview.
    if !held {
        *drag = None;
    }
    drag.as_ref().map(|d| d.col).or(hovered)
}

/// Highlights the divider of `active` (hovered or dragged) over the full header height.
pub(super) fn paint_active(ui: &egui::Ui, hdr: Rect, cols: &Columns, cfg: &GraphColumns, active: Option<ResizableColumn>) {
    let Some(active) = active else { return };
    if let Some((_, x)) = cols.grips(cfg).find(|(c, _)| *c == active) {
        ui.painter().line_segment([pos2(x, hdr.top()), pos2(x, hdr.bottom())], Stroke::new(2.0_f32, color::accent()));
    }
}
