//! The mouse on the terminal grid: the wheel scrolls the history, dragging the left button
//! selects text in reading order (copied when the drag ends, with Copy on select), and on Linux
//! a middle click pastes.

use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::Selection;
use crate::terminal::TerminalMsg;
use eframe::egui::{self, Event, MouseWheelUnit, PointerButton, Pos2};
use trellis_core::terminal::TermPoint;

/// Rows one wheel notch scrolls.
const ROWS_PER_NOTCH: f32 = 3.0;

/// Where the grid's cells are: its top-left corner, a cell's size and the screen's size.
pub(super) struct Geometry {
    pub origin: Pos2,
    pub cw: f32,
    pub rh: f32,
    pub rows: u16,
    pub cols: u16,
}

/// The cell boundary nearest `pos`, as (row, col): the row under it and the nearest column
/// boundary, both kept within the screen (col 0..=cols).
pub(super) fn cell_at(pos: Pos2, origin: Pos2, cw: f32, rh: f32, rows: u16, cols: u16) -> (u16, u16) {
    let r = ((pos.y - origin.y) / rh).floor().clamp(0.0, f32::from(rows.saturating_sub(1)));
    let c = ((pos.x - origin.x) / cw).round().clamp(0.0, f32::from(cols));
    (r as u16, c as u16)
}

/// The whole rows this frame's wheel `events` scroll back (forward when negative): three per
/// notch, points divided by the row height `rh` (the remainder carried in `rem`), a page per
/// screen of `rows`. Ctrl and Cmd wheels are the app's zoom, not scrolling.
pub(super) fn wheel_rows(events: &[Event], rh: f32, rows: u16, rem: &mut f32) -> i32 {
    let mut total = *rem;
    for e in events {
        let Event::MouseWheel { unit, delta, modifiers } = e else { continue };
        if modifiers.ctrl || modifiers.command {
            continue;
        }
        total += match unit {
            MouseWheelUnit::Line => delta.y * ROWS_PER_NOTCH,
            MouseWheelUnit::Point => delta.y / rh.max(1.0),
            MouseWheelUnit::Page => delta.y * f32::from(rows),
        };
    }
    let whole = total.trunc();
    *rem = total - whole;
    whole as i32
}

/// The wheel, drag selection and middle click on the grid `resp` of session `index`.
pub(super) fn handle(ui: &egui::Ui, resp: &egui::Response, st: &mut AppState, index: usize, g: &Geometry, msgs: &mut Vec<Msg>) {
    let copy_on_select = st.settings.terminal.copy_on_select;
    if st.settings.terminal.middle_click_pastes() && resp.clicked_by(PointerButton::Middle) {
        msgs.push(Msg::Terminal(TerminalMsg::Paste));
    }
    let session = &mut st.terminal.sessions[index];
    if resp.hovered() {
        let rows = ui.input(|i| wheel_rows(&i.events, g.rh, g.rows, &mut st.terminal.wheel_rem));
        if session.screen.alternate() {
            st.terminal.wheel_rem = 0.0;
        } else if rows != 0 {
            msgs.push(Msg::Terminal(TerminalMsg::Scroll(rows)));
        }
    }
    let screen = &session.screen;
    let point = |pos: Pos2| {
        let (r, col) = cell_at(pos, g.origin, g.cw, g.rh, g.rows, g.cols);
        TermPoint { line: screen.top_line() + u64::from(r), col }
    };
    let pos = resp.interact_pointer_pos();
    if resp.drag_started_by(PointerButton::Primary) {
        if let Some(pos) = pos {
            let from = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
            session.selection = Some(Selection { anchor: point(from), head: point(pos), epoch: screen.epoch() });
        }
        resp.request_focus();
    } else if resp.dragged_by(PointerButton::Primary) {
        let head = pos.map(point);
        if let (Some(sel), Some(head)) = (session.selection.as_mut(), head) {
            sel.head = head;
        }
    }
    if copy_on_select && resp.drag_stopped_by(PointerButton::Primary) && session.selection.is_some_and(|s| s.range(screen).is_some()) {
        msgs.push(Msg::Terminal(TerminalMsg::CopySelection { keep: true }));
    }
    if resp.clicked() {
        session.selection = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Modifiers, pos2, vec2};

    #[test]
    fn cells_round_to_the_nearest_boundary() {
        let o = pos2(10.0, 20.0);
        let at = |x, y| cell_at(pos2(x, y), o, 8.0, 16.0, 24, 80);
        assert_eq!(at(10.0, 20.0), (0, 0));
        assert_eq!(at(13.9, 35.9), (0, 0), "left half of the first cell");
        assert_eq!(at(14.1, 36.1), (1, 1), "right half: the boundary after it");
        assert_eq!(at(0.0, 0.0), (0, 0), "above and left of the grid");
        assert_eq!(at(5000.0, 5000.0), (23, 80), "past the last cell");
    }

    fn wheel(unit: MouseWheelUnit, y: f32, modifiers: Modifiers) -> Event {
        Event::MouseWheel { unit, delta: vec2(0.0, y), modifiers }
    }

    #[test]
    fn wheel_steps() {
        let none = Modifiers::NONE;
        let mut rem = 0.0;
        assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Line, 1.0, none)], 16.0, 24, &mut rem), 3);
        assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Line, -2.0, none)], 16.0, 24, &mut rem), -6);
        assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Page, 1.0, none)], 16.0, 24, &mut rem), 24);
        // Points: 10 px of a 16 px row is carried until a whole row has come.
        assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Point, 10.0, none)], 16.0, 24, &mut rem), 0);
        assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Point, 10.0, none)], 16.0, 24, &mut rem), 1);
        assert!((rem - 0.25).abs() < 1e-4, "{rem}");
        let mut rem = 0.0;
        for zoom in [Modifiers::CTRL, Modifiers::COMMAND, Modifiers::MAC_CMD | Modifiers::COMMAND] {
            assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Line, 1.0, zoom)], 16.0, 24, &mut rem), 0, "{zoom:?}");
        }
        assert_eq!(rem, 0.0);
    }
}
