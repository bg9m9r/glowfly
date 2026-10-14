//! The terminal's screen grid: painting the active session's cells and cursor, reporting the
//! grid size, and forwarding the keyboard while it has focus.

use super::colors::span_colors;
use super::input;
use crate::msg::Msg;
use crate::state::AppState;
use crate::terminal::TerminalMsg;
use crate::ui::theme::color;
use eframe::egui::{self, Align2, EventFilter, FontId, Id, Rect, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::terminal::TermSpan;

/// Space between the grid and the panel's edges.
const PAD: f32 = 4.0;
const MIN_ROWS: u16 = 2;
const MIN_COLS: u16 = 10;

/// Draws session `index` in the rest of `ui`.
pub(super) fn show(ui: &mut egui::Ui, st: &mut AppState, index: usize, msgs: &mut Vec<Msg>) {
    let font = FontId::monospace(st.settings.diff_font_px());
    let cw = ui.fonts(|f| f.glyph_width(&font, 'M'));
    let rh = ui.fonts(|f| f.row_height(&font)).round();
    let id = Id::new("trellis-terminal-grid");
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Terminal screen"));

    if st.terminal.focus_requested || resp.clicked() {
        resp.request_focus();
        st.terminal.focus_requested = false;
    }
    if resp.clicked_elsewhere() {
        resp.surrender_focus();
    }
    let focused = resp.has_focus();
    st.terminal.focused = focused;
    if !focused {
        // Its key's release now goes elsewhere.
        st.terminal.clip_pending = None;
    }
    if focused {
        // Tab, arrows and Esc belong to the shell, not to egui's focus navigation.
        let filter = EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
        ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
        let inputs = input::collect(&ui.input(|i| i.events.clone()), &mut st.terminal.clip_pending);
        if !inputs.is_empty() {
            msgs.push(Msg::Terminal(TerminalMsg::Input(inputs)));
        }
    }

    let rows = (((rect.height() - 2.0 * PAD) / rh).floor().max(0.0) as u16).max(MIN_ROWS);
    let cols = (((rect.width() - 2.0 * PAD) / cw).floor().max(0.0) as u16).max(MIN_COLS);
    let session = &st.terminal.sessions[index];
    if session.screen.size() != (rows, cols) || st.terminal.grid != Some((rows, cols)) {
        msgs.push(Msg::Terminal(TerminalMsg::Resize { rows, cols }));
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color::input());
    let origin = rect.min + vec2(PAD, PAD);
    let cell = |r: u16, c: u16| origin + vec2(f32::from(c) * cw, f32::from(r) * rh);
    for (r, row) in session.screen.rows().iter().enumerate() {
        let r = r as u16;
        for span in &row.spans {
            let at = cell(r, span.col);
            let (fg, bg) = span_colors(&span.style);
            if let Some(bg) = bg {
                painter.rect_filled(Rect::from_min_size(at, vec2(f32::from(span.width) * cw, rh)), 0.0, bg);
            }
            paint_text(&painter, at, span, cw, &font, fg);
            if span.style.underline {
                let y = at.y + rh - 1.5;
                painter.line_segment([pos2(at.x, y), pos2(at.x + f32::from(span.width) * cw, y)], Stroke::new(1.0_f32, fg));
            }
        }
    }
    if let Some((r, c)) = session.screen.cursor().filter(|_| session.alive()) {
        let block = Rect::from_min_size(cell(r, c), vec2(cw, rh));
        if focused {
            painter.rect_filled(block, 0.0, color::accent().gamma_multiply(0.6));
        } else {
            painter.rect_stroke(block, 0.0, Stroke::new(1.0_f32, color::accent()), StrokeKind::Inside);
        }
    }
}

/// Paints `span` from `at`, each cell at its own column (an ASCII run in one piece), so a
/// fallback font's glyph widths or a cell's combining marks can't shift the columns after it.
/// Bold is the text again one pixel to the right.
fn paint_text(painter: &egui::Painter, at: egui::Pos2, span: &TermSpan, cw: f32, font: &FontId, fg: egui::Color32) {
    let offsets: &[f32] = if span.style.bold { &[0.0, 1.0] } else { &[0.0] };
    for dx in offsets {
        for (col, text) in span.cells() {
            let x = at.x + dx + f32::from(col) * cw;
            painter.text(pos2(x, at.y), Align2::LEFT_TOP, text, font.clone(), fg);
        }
    }
}
