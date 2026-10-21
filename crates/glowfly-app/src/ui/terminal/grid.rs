//! The terminal's screen grid: painting the active tab's cells, selection and cursor, reporting
//! the grid size, the mouse (mouse.rs), the right-click menu, and forwarding the keyboard while
//! it has focus (Shift with Page Up/Down, Home and End scroll the main screen's history).

use super::colors::span_colors;
use super::mouse::{self, Geometry};
use super::{input, paint};
use crate::msg::Msg;
use crate::state::AppState;
use crate::terminal::TerminalMsg;
use crate::ui::theme::color;
use eframe::egui::{self, Align2, EventFilter, FontId, Id, Rect, Sense, Stroke, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::terminal::TermSpan;

/// Space between the grid and the panel's edges.
const PAD: f32 = 4.0;
const MIN_ROWS: u16 = 2;
const MIN_COLS: u16 = 10;

/// Draws session `index` in the rest of `ui`.
pub(super) fn show(ui: &mut egui::Ui, st: &mut AppState, index: usize, msgs: &mut Vec<Msg>) {
    let font = FontId::monospace(st.settings.terminal.font_px());
    let (cw, rh) = ui.fonts(|f| super::metrics::cell_size(f, &font));
    let id = Id::new("trellis-terminal-grid");
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Terminal screen"));

    if st.terminal.focus_requested || resp.clicked() {
        resp.request_focus();
        st.terminal.focus_requested = false;
    }
    if resp.clicked_elsewhere() {
        resp.surrender_focus();
    }
    let origin = rect.min + vec2(PAD, PAD);
    let (rows_now, cols_now) = st.terminal.sessions[index].screen.size();
    let g = Geometry { origin, cw, rh, rows: rows_now, cols: cols_now };
    mouse::handle(ui, &resp, st, index, &g, msgs);
    let focused = resp.has_focus();
    st.terminal.focused = focused;
    if !focused {
        // Its key's release now goes elsewhere.
        st.terminal.clip_pending = None;
    }
    let session = &st.terminal.sessions[index];
    let selected = session.selection.and_then(|s| s.range(&session.screen));
    if focused {
        // Tab, arrows and Esc belong to the shell, not to egui's focus navigation.
        let filter = EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
        ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
        let opts = input::Opts {
            // With Copy on select the text was copied when it was selected, so the copy key
            // goes to the shell.
            has_selection: selected.is_some() && !st.settings.terminal.copy_on_select,
            history_keys: !session.screen.alternate(),
            alt_is_meta: !cfg!(target_os = "macos"),
        };
        let got = input::collect(&ui.input(|i| i.events.clone()), &mut st.terminal.clip_pending, opts);
        if got.copy {
            msgs.push(Msg::Terminal(TerminalMsg::CopySelection { keep: false }));
        }
        for step in got.scroll {
            msgs.push(Msg::Terminal(TerminalMsg::ScrollPage(step)));
        }
        if !got.inputs.is_empty() {
            msgs.push(Msg::Terminal(TerminalMsg::Input(got.inputs)));
        }
    }
    let session = &st.terminal.sessions[index];
    resp.context_menu(|ui| super::context_menu(ui, session, selected.is_some(), msgs));

    let rows = (((rect.height() - 2.0 * PAD) / rh).floor().max(0.0) as u16).max(MIN_ROWS);
    let cols = (((rect.width() - 2.0 * PAD) / cw).floor().max(0.0) as u16).max(MIN_COLS);
    if session.screen.size() != (rows, cols) || st.terminal.grid != Some((rows, cols)) {
        msgs.push(Msg::Terminal(TerminalMsg::Resize { rows, cols }));
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color::input());
    let cell = |r: u16, c: u16| origin + vec2(f32::from(c) * cw, f32::from(r) * rh);
    let screen_rows = session.screen.rows();
    for (r, row) in screen_rows.iter().enumerate() {
        for span in &row.spans {
            if let (_, Some(bg)) = span_colors(&span.style) {
                painter.rect_filled(Rect::from_min_size(cell(r as u16, span.col), vec2(f32::from(span.width) * cw, rh)), 0.0, bg);
            }
        }
    }
    if let Some(range) = selected {
        paint::selection(&painter, range, session.screen.top_line(), (rows_now, cols_now), (cw, rh), cell);
    }
    for (r, row) in screen_rows.iter().enumerate() {
        for span in &row.spans {
            let at = cell(r as u16, span.col);
            let (fg, _) = span_colors(&span.style);
            paint_text(&painter, at, span, cw, &font, fg);
            if span.style.underline {
                let y = at.y + rh - 1.5;
                painter.line_segment([pos2(at.x, y), pos2(at.x + f32::from(span.width) * cw, y)], Stroke::new(1.0_f32, fg));
            }
        }
    }
    if let Some((r, c)) = session.screen.cursor().filter(|_| session.alive()) {
        paint::cursor(&painter, Rect::from_min_size(cell(r, c), vec2(cw, rh)), st.settings.terminal.cursor, focused);
    }
    if st.settings.terminal.dim_unfocused && !focused {
        paint::dim(&painter, rect);
    }
    paint::thumb(&painter, rect, session.screen.view(), session.screen.history_len(), rows_now);
}

/// Paints `span` from `at`. An ASCII run is one galley: `cw` is the pixel-rounded advance epaint
/// places each of its glyphs at (metrics.rs), so they land on their cells. Fallback-font and
/// combining-mark cells are still placed one by one, so their widths can't shift the columns
/// after them. Bold is the text again one pixel to the right.
fn paint_text(painter: &egui::Painter, at: egui::Pos2, span: &TermSpan, cw: f32, font: &FontId, fg: egui::Color32) {
    let offsets: &[f32] = if span.style.bold { &[0.0, 1.0] } else { &[0.0] };
    for dx in offsets {
        for (col, text) in span.cells() {
            let x = at.x + dx + f32::from(col) * cw;
            painter.text(pos2(x, at.y), Align2::LEFT_TOP, text, font.clone(), fg);
        }
    }
}
