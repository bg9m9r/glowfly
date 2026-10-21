//! The open part of an activity row, under its title: a failure's cause in a full sentence,
//! its fix (once) and Copy, then the output in a quiet monospace block coloured by kind of line
//! (the last `OUTPUT_LINES` of it); and the sub-rows of an open routine group.

use super::activity_rows::{RowLook, TITLE_X, paint_row};
use super::theme::{color, type_scale};
use super::widgets::buttons;
use crate::logs::LogsMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::activity_rows::{copy_text, output_of};
use crate::state::logs::LogEntry;
use crate::ui::theme::tokens::radius;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, Ui};
use trellis_core::logs::{OutputLine, classify_line, output_lines};

/// The most output lines an open row draws; Copy has them all.
pub(super) const OUTPUT_LINES: usize = 200;
/// Room under the open part.
const BOTTOM: f32 = 8.0;

/// Draws `e`'s open part, indented to its title.
pub(super) fn show(ui: &mut Ui, st: &AppState, e: &LogEntry, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let indent = TITLE_X;
    ui.horizontal_top(|ui| {
        ui.add_space(indent);
        ui.vertical(|ui| {
            ui.set_max_width((ui.available_width() - 8.0).max(0.0));
            ui.spacing_mut().item_spacing.y = 6.0;
            let failed_run = e.run.as_ref().filter(|_| !e.ok);
            if let Some(cause) = failed_run.map(|r| r.summary.as_str()).filter(|s| !s.is_empty()) {
                ui.add(egui::Label::new(RichText::new(cause).color(color::text())).wrap());
            }
            ui.horizontal(|ui| {
                if let Some(label) = st.logs.fix_label(e.id)
                    && ui.add(buttons::primary(label).small()).clicked()
                {
                    msgs.push(Msg::Logs(LogsMsg::Fix(e.id)));
                }
                if ui.add(buttons::secondary(t.tr("logs.activity.copy.button")).small()).clicked() {
                    msgs.push(Msg::Copy(copy_text(e)));
                }
            });
            output(ui, st, output_of(e));
            ui.add_space(BOTTOM - 6.0);
        });
    });
}

/// The colour of an output line of kind `k`.
fn line_color(k: OutputLine) -> Color32 {
    match k {
        OutputLine::Command => color::text(),
        OutputLine::Error => color::removed(),
        OutputLine::Warning => color::warn(),
        OutputLine::Hint => color::faint(),
        OutputLine::Remote | OutputLine::Plain => color::weak(),
    }
}

/// The output block: the last `OUTPUT_LINES` lines of `text` (blank ones at the end left
/// out), under a note when some were left out. Nothing for no output.
fn output(ui: &mut Ui, st: &AppState, text: &str) {
    let mut lines: Vec<&str> = output_lines(text).collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return;
    }
    let skip = lines.len().saturating_sub(OUTPUT_LINES);
    if skip > 0 {
        let note = st.settings.lang().trn("logs.activity.cut", OUTPUT_LINES, &[]);
        ui.label(RichText::new(note).size(type_scale::SMALL).color(color::faint()));
    }
    let font = type_scale::mono(type_scale::SMALL);
    let mut job = LayoutJob::default();
    for (n, line) in lines[skip..].iter().enumerate() {
        let sep = if n == 0 { "" } else { "\n" };
        let format = TextFormat { font_id: font.clone(), color: line_color(classify_line(line)), ..Default::default() };
        job.append(&format!("{sep}{line}"), 0.0, format);
    }
    egui::Frame::new()
        .fill(color::bg())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(CornerRadius::same(radius::MD))
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(job).wrap().selectable(false));
        });
}

/// One run of an open routine group: a 28 px faint row with its time, duration and summary.
pub(super) fn sub_row(ui: &mut Ui, st: &AppState, e: &LogEntry) {
    let summary = e.run.as_ref().map_or("", |r| r.summary.as_str());
    let time = trellis_core::dates::relative(st.settings.lang(), st.view.now - e.at);
    let look = RowLook {
        title: &time,
        title_color: color::faint(),
        summary,
        took_ms: e.run.as_ref().and_then(|r| r.took_ms),
        at: e.at,
        time_tip: st.settings.format_verbose(e.at, st.view.utc_offset),
        open: None,
        sub: true,
    };
    paint_row(ui, st, &look, |_, _| {});
}
