//! The mouse on the terminal grid: the wheel scrolls the history (on the alternate screen it
//! sends arrow keys to the full-screen program), dragging the left button selects text in
//! reading order and scrolls the history past the top or bottom edge, a double-click selects a
//! word and a triple-click a line (each becoming the PRIMARY selection on Linux, and copied
//! with Copy on select), and on Linux a middle click pastes the PRIMARY selection.

use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::Selection;
use crate::terminal::TerminalMsg;
use eframe::egui::{self, Event, MouseWheelUnit, PointerButton, Pos2};
use glowfly_core::terminal::{KeyMods, TermInput, TermKey, TermPoint};

/// Rows one wheel notch scrolls.
const ROWS_PER_NOTCH: f32 = 3.0;

/// Rows a second a drag just past the grid's edge scrolls; each row's height further out adds
/// as many again.
const EDGE_ROWS_PER_SEC: f32 = 8.0;

/// The fastest a drag past the edge scrolls, in rows a second.
const EDGE_MAX_ROWS_PER_SEC: f32 = 200.0;

/// The most arrow keys one frame's wheel sends a full-screen program.
const MAX_WHEEL_KEYS: i32 = 100;

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

/// The cell under `pos`, as (row, col), kept within the screen (col 0..cols).
pub(super) fn cell_under(pos: Pos2, g: &Geometry) -> (u16, u16) {
    let r = ((pos.y - g.origin.y) / g.rh).floor().clamp(0.0, f32::from(g.rows.saturating_sub(1)));
    let c = ((pos.x - g.origin.x) / g.cw).floor().clamp(0.0, f32::from(g.cols.saturating_sub(1)));
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

/// What the wheel's `rows` (back when positive) send a full-screen program: an Up arrow per
/// row back, a Down arrow per row forward. They are encoded in the program's cursor-keys mode
/// when they are sent (update/terminal.rs).
pub(super) fn wheel_keys(rows: i32) -> Vec<TermInput> {
    let key = if rows > 0 { TermKey::Up } else { TermKey::Down };
    let n = rows.unsigned_abs().min(MAX_WHEEL_KEYS.unsigned_abs()) as usize;
    vec![TermInput::Key { key, mods: KeyMods::default() }; n]
}

/// The whole rows a selection drag at height `y` scrolls in a frame of `dt` seconds: back
/// (positive) above `top`, forward below `bottom`, faster the further out, the part of a row
/// carried in `rem`. Inside the grid nothing scrolls and `rem` is dropped.
pub(super) fn edge_rows(y: f32, top: f32, bottom: f32, rh: f32, dt: f32, rem: &mut f32) -> i32 {
    let past = if y < top {
        top - y
    } else if y > bottom {
        bottom - y
    } else {
        *rem = 0.0;
        return 0;
    };
    let speed = (EDGE_ROWS_PER_SEC * (1.0 + past.abs() / rh.max(1.0))).min(EDGE_MAX_ROWS_PER_SEC);
    let total = *rem + past.signum() * speed * dt.clamp(0.0, 0.1);
    let whole = total.trunc();
    *rem = total - whole;
    whole as i32
}

/// The wheel, drag selection, word and line clicks, and middle click on the grid `resp` of
/// session `index`.
pub(super) fn handle(ui: &egui::Ui, resp: &egui::Response, st: &mut AppState, index: usize, g: &Geometry, msgs: &mut Vec<Msg>) {
    if st.settings.terminal.middle_click_pastes() && resp.clicked_by(PointerButton::Middle) {
        msgs.push(Msg::Terminal(TerminalMsg::PastePrimary));
    }
    let session = &mut st.terminal.sessions[index];
    if resp.hovered() {
        let rows = ui.input(|i| wheel_rows(&i.events, g.rh, g.rows, &mut st.terminal.wheel_rem));
        if !session.screen.alternate() {
            if rows != 0 {
                msgs.push(Msg::Terminal(TerminalMsg::Scroll(rows)));
            }
        } else if session.screen.mouse_reporting() {
            // The program asked for mouse events, which the terminal doesn't report yet.
            st.terminal.wheel_rem = 0.0;
        } else if rows != 0 {
            msgs.push(Msg::Terminal(TerminalMsg::Input(wheel_keys(rows))));
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
        st.terminal.edge_rem = 0.0;
        resp.request_focus();
    } else if resp.dragged_by(PointerButton::Primary)
        && let Some(pos) = pos
    {
        if let Some(sel) = session.selection.as_mut() {
            sel.head = point(pos);
        }
        // The last row, or the grid's edge while a smaller screen size is on its way.
        let bottom = (g.origin.y + f32::from(g.rows) * g.rh).min(resp.rect.max.y);
        if !screen.alternate() && !(g.origin.y..=bottom).contains(&pos.y) {
            let dt = ui.input(|i| i.stable_dt);
            let rows = edge_rows(pos.y, g.origin.y, bottom, g.rh, dt, &mut st.terminal.edge_rem);
            if rows != 0 {
                msgs.push(Msg::Terminal(TerminalMsg::Scroll(rows)));
            }
            // Keep scrolling while the pointer is held still out there.
            ui.ctx().request_repaint();
        } else {
            st.terminal.edge_rem = 0.0;
        }
    }
    let mut picked = false;
    if let Some(pos) = pos.filter(|_| resp.clicked()) {
        let (r, col) = cell_under(pos, g);
        let line = session.screen.top_line() + u64::from(r);
        let range = if resp.triple_clicked() {
            Some(session.screen.line_at(line))
        } else if resp.double_clicked() {
            Some(session.screen.word_at(TermPoint { line, col }))
        } else {
            None
        };
        let epoch = session.screen.epoch();
        session.selection = range.map(|(anchor, head)| Selection { anchor, head, epoch });
        picked = range.is_some();
    }
    let ended = picked || resp.drag_stopped_by(PointerButton::Primary);
    if ended && session.selection.is_some_and(|s| s.range(&session.screen).is_some()) {
        msgs.push(Msg::Terminal(TerminalMsg::Selected));
    }
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
