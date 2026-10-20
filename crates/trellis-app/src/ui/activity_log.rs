//! The logs panel at the bottom, on the panel surface: a 40 px header (the Activity, Errors and
//! Performance tabs on the left, quiet icon buttons on the right, a hairline under it), then
//! the active tab's activity log (ui/activity_rows.rs) or the end of the error or performance
//! log, read by the runner.

use super::pane_status::{self, Failure};
use super::panel_size;
use super::theme::color;
use crate::logs::{LogKind, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::panel_layout::{self, ACTIVITY, MAIN_MIN_HEIGHT};
use crate::state::tip::Tip;
use crate::ui::widgets::Icon;
use crate::ui::widgets::ghost::{self, divider, ghost, icon_tip};
use crate::ui::widgets::segmented::segmented;
use eframe::egui::{self, Align, Layout, Rect, RichText, Sense, Stroke, TextStyle, Ui, UiBuilder, pos2, vec2};
use trellis_core::settings::SizedPanel;

pub(super) const HEADER_H: f32 = 40.0;
/// The panel's side margin, which the hairline runs through.
pub(super) const SIDE: f32 = 8.0;
/// The right cluster: three buttons, the gaps between its four items and the divider.
const RIGHT_W: f32 = 3.0 * ghost::HEIGHT + 3.0 * ghost::SPACE + ghost::DIVIDER_W;

/// The panel's frame: its surface, 8 px at the sides and 6 px under the list; the header starts
/// at the top edge.
fn frame() -> egui::Frame {
    let side = SIDE as i8;
    egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin { left: side, right: side, top: 0, bottom: 6 })
}

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !st.view.show_log {
        return;
    }
    // The terminal is drawn under the same room later: leave it its minimum too.
    let terminal = st.terminal.open && st.snap.is_some() && !st.dashboard.open;
    let keep = MAIN_MIN_HEIGHT + if terminal { panel_layout::TERMINAL.min } else { 0.0 };
    let fit = panel_layout::fit_one(ctx.available_rect().height(), keep, ACTIVITY, st.settings.panel_sizes.get(SizedPanel::Activity));
    let id = egui::Id::new(panel_size::ACTIVITY_ID);
    panel_size::pin(ctx, id, fit.size, st.view.panel_drag == Some(SizedPanel::Activity));
    let panel = egui::TopBottomPanel::bottom(id).resizable(true).default_height(fit.size).height_range(fit.lo..=fit.hi).frame(frame());
    let resp = panel.show(ctx, |ui| {
        header(ui, st, msgs);
        match st.logs.tab {
            LogKind::Activity => super::activity_rows::show(ui, st, msgs),
            kind => file(ui, st, kind, msgs),
        }
    });
    panel_size::track(ctx, st, SizedPanel::Activity, id, resp.response.rect.height(), msgs);
}

/// The 40 px header: the tabs on the left, the buttons on the right, a hairline under it.
fn header(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
    let right = Rect::from_min_max(pos2(rect.right() - RIGHT_W, rect.top()), rect.max);
    let mut r = ui.new_child(UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)));
    r.spacing_mut().item_spacing.x = ghost::SPACE;
    buttons(&mut r, st, msgs);
    let left = Rect::from_min_max(rect.min, pos2(right.left() - SIDE, rect.bottom()));
    let mut l = ui.new_child(UiBuilder::new().max_rect(left).layout(Layout::left_to_right(Align::Center)));
    let t = st.settings.lang();
    let tabs: Vec<(LogKind, &str, &str)> = LogKind::ALL.iter().map(|&k| (k, k.name(t), k.tip(t))).collect();
    if let Some(kind) = segmented(&mut l, st.logs.tab, &tabs) {
        msgs.push(Msg::Logs(LogsMsg::Show(kind)));
    }
    let x = rect.x_range().expand(SIDE);
    ui.painter().hline(x, rect.bottom() - 0.5, Stroke::new(1.0_f32, color::border()));
}

/// The right-hand buttons, drawn right to left: Hide panel | Open folder, then Clear on the
/// activity tab or Reload on a file tab.
fn buttons(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let keys = &st.keymap;
    let hide = t.tr("logs.header.hide");
    let hide_clicked = icon_tip(ghost(ui, Some(Icon::ChevronDown), None, hide, true), keys, t, Tip::new(hide)).clicked();
    divider(ui);
    let open = t.tr("logs.header.open_folder");
    let body = match &st.logs.dir {
        Some(dir) => trellis_core::native_path::shown(dir),
        None => t.tr("logs.header.open_folder.tip").to_owned(),
    };
    if icon_tip(ghost(ui, Some(Icon::Folder), None, open, true), keys, t, Tip::new(open).body(body)).clicked() {
        msgs.push(Msg::Logs(LogsMsg::OpenFolder));
    }
    let (icon, name, tip, msg) = match st.logs.tab {
        LogKind::Activity => (Icon::Eraser, t.tr("logs.header.clear"), t.tr("logs.header.clear.tip"), Msg::ClearLog),
        kind => (Icon::Refresh, t.tr("logs.header.reload"), t.tr("logs.header.reload.tip"), Msg::Logs(LogsMsg::Show(kind))),
    };
    if icon_tip(ghost(ui, Some(icon), None, name, true), keys, t, Tip::new(name).body(tip)).clicked() {
        msgs.push(msg);
    }
    if hide_clicked {
        st.view.show_log = false;
    }
}

/// The end of the error or performance log, one monospace row per line.
fn file(ui: &mut Ui, st: &AppState, kind: LogKind, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.add_space(4.0);
    let view = st.logs.file.as_ref().filter(|v| v.kind == kind);
    let tail = match view.and_then(|v| v.tail.as_ref()) {
        None => return pane_status::loading_line(ui, t.tr("logs.file.reading")),
        Some(Err(e)) => {
            let f = Failure { title: t.tr("logs.file.failed"), error: e, retry: Some(Msg::Logs(LogsMsg::Show(kind))) };
            return pane_status::failed_line(ui, t, f, msgs);
        }
        Some(Ok(tail)) if tail.lines.is_empty() => {
            ui.label(RichText::new(t.tr("logs.file.empty")).color(color::faint()));
            return;
        }
        Some(Ok(tail)) => tail,
    };
    let note = usize::from(tail.truncated);
    let row_height = ui.text_style_height(&TextStyle::Monospace) + ui.spacing().item_spacing.y;
    egui::ScrollArea::both().stick_to_bottom(true).auto_shrink([false, false]).show_rows(
        ui,
        row_height,
        tail.lines.len() + note,
        |ui, rows| {
            for i in rows {
                let text = match i.checked_sub(note) {
                    None => RichText::new(t.tr("logs.file.cut")).color(color::faint()),
                    Some(n) => {
                        let line = &tail.lines[n];
                        let c = if line.contains(" ERROR ") {
                            color::removed()
                        } else if line.contains(" WARN ") {
                            color::warn()
                        } else {
                            color::text()
                        };
                        RichText::new(line).monospace().color(c)
                    }
                };
                ui.add(egui::Label::new(text).extend());
            }
        },
    );
}
