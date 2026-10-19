//! The logs panel at the bottom: the active tab's activity log, with earlier sessions' entries
//! in dimmed groups, and the ends of the error and performance logs, read by the runner.

use super::pane_status::{self, Failure};
use super::panel_size;
use super::theme::color;
use crate::logs::{LogKind, LogsMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::panel_layout::{self, ACTIVITY, MAIN_MIN_HEIGHT};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText, TextStyle, Ui};
use trellis_core::settings::SizedPanel;

/// The first row of a file tab whose start was left out.
const CUT_NOTE: &str = "Showing the end of the file. Open the logs folder for all of it.";

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
    let resp = egui::TopBottomPanel::bottom(id).resizable(true).default_height(fit.size).height_range(fit.lo..=fit.hi).show(ctx, |ui| {
        header(ui, st, msgs);
        match st.logs.tab {
            LogKind::Activity => activity(ui, st),
            kind => file(ui, st, kind, msgs),
        }
    });
    panel_size::track(ctx, st, SizedPanel::Activity, id, resp.response.rect.height(), msgs);
}

fn header(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        for kind in LogKind::ALL {
            if ui.selectable_label(st.logs.tab == kind, kind.label()).clicked() {
                msgs.push(Msg::Logs(LogsMsg::Show(kind)));
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("Hide").clicked() {
                st.view.show_log = false;
            }
            let tip = match &st.logs.dir {
                Some(dir) => trellis_core::native_path::shown(dir),
                None => "The folder Trellis keeps its logs in".to_owned(),
            };
            if ui.small_button("Open folder").on_hover_text(tip).clicked() {
                msgs.push(Msg::Logs(LogsMsg::OpenFolder));
            }
            match st.logs.tab {
                LogKind::Activity => {
                    if ui.small_button("Clear").clicked() {
                        msgs.push(Msg::ClearLog);
                    }
                }
                kind => {
                    if ui.small_button("Reload").clicked() {
                        msgs.push(Msg::Logs(LogsMsg::Show(kind)));
                    }
                }
            }
        });
    });
}

fn group_header(ui: &mut Ui, text: String) {
    ui.label(type_scale::section_text(ui, text));
}

/// The activity log: earlier sessions' entries first, each session under its start time, then
/// this session's under "This session" (only when there are earlier ones).
fn activity(ui: &mut Ui, st: &AppState) {
    let any_earlier = st.log.first().is_some_and(|e| e.earlier.is_some());
    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        let mut session = None;
        let mut current = false;
        for entry in &st.log {
            let c = match entry.earlier {
                Some(e) => {
                    if session != Some(e.session) {
                        session = Some(e.session);
                        let at = st.settings.format_verbose(e.session, st.view.utc_offset);
                        group_header(ui, format!("Earlier session · {at}"));
                    }
                    if entry.ok { color::weak() } else { color::removed().gamma_multiply(0.6) }
                }
                None => {
                    if any_earlier && !current {
                        current = true;
                        group_header(ui, "This session".to_owned());
                    }
                    if entry.ok { color::text() } else { color::removed() }
                }
            };
            ui.label(RichText::new(&entry.text).color(c));
            ui.separator();
        }
    });
}

/// The end of the error or performance log, one monospace row per line.
fn file(ui: &mut Ui, st: &AppState, kind: LogKind, msgs: &mut Vec<Msg>) {
    let view = st.logs.file.as_ref().filter(|v| v.kind == kind);
    let tail = match view.and_then(|v| v.tail.as_ref()) {
        None => return pane_status::loading_line(ui, "Reading…"),
        Some(Err(e)) => {
            let f = Failure { title: "Couldn't read this log", error: e, retry: Some(Msg::Logs(LogsMsg::Show(kind))) };
            return pane_status::failed_line(ui, st.settings.lang(), f, msgs);
        }
        Some(Ok(t)) if t.lines.is_empty() => {
            ui.label(RichText::new("Nothing has been logged here yet.").color(color::faint()));
            return;
        }
        Some(Ok(t)) => t,
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
                    None => RichText::new(CUT_NOTE).color(color::faint()),
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
