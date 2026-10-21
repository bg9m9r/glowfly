//! The selected state's preview: its label, "<subject> · <sha> · <date>", what Restore would
//! move (and, when nothing keeps its commit any more, that only the reflog does), up to 8
//! first-parent commits from it painted as a short lane in the subject's colour, Restore… and
//! Create branch here…, and git's raw reflog entry. The lane is shared with `lost_preview.rs`.

use crate::journal::JournalMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::{JournalView, moves_sentence, row_label, short, subject_name, unreachable_line};
use crate::state::journal_restore;
use crate::ui::pane_status;
use crate::ui::theme::branch_colours::line;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::{buttons, paint_cut_text, reason};
use eframe::egui::{self, Align, Align2, Color32, RichText, Sense, Ui, pos2, vec2};
use trellis_core::journal::{CommitLine, Row, Subject};
use trellis_core::repo::Snapshot;

const INSET: f32 = 16.0;
const LANE_ROW_H: f32 = 28.0;
const DOT_R: f32 = 3.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(view), Some(snap)) = (&st.journal, &st.snap) else { return };
    let t = st.settings.lang();
    let Some(row) = view.selected_row() else {
        ui.add_space(32.0);
        ui.vertical_centered(|ui| ui.label(RichText::new(t.tr("journal.select_hint")).color(color::weak())));
        return;
    };
    egui::ScrollArea::vertical().id_salt("journal-preview").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(INSET as i8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            pane(ui, st, view, snap, row, msgs);
        });
    });
}

fn pane(ui: &mut Ui, st: &AppState, view: &JournalView, snap: &Snapshot, row: &Row, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.spacing_mut().item_spacing.y = 6.0;
    let title = RichText::new(row_label(t, row)).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text());
    ui.add(egui::Label::new(title).wrap());
    let date = st.settings.format_verbose(row.at, st.view.utc_offset);
    let meta = format!("{} · {} · {date}", subject_name(&row.subject), short(row.oid));
    ui.label(RichText::new(meta).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
    let preview = view.preview_of(&row.key());
    if let Some(moves) = moves_sentence(t, row, preview, snap) {
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(moves).color(color::text())).wrap());
    }
    if row.unreachable && row.summary.is_some() {
        ui.add(egui::Label::new(RichText::new(unreachable_line(t)).color(color::text())).wrap());
    }
    ui.add_space(10.0);
    ui.label(RichText::new(t.tr("journal.preview.history")).font(type_scale::section(ui)).color(color::weak()));
    match view.preview.as_ref().filter(|p| p.key == row.key()).and_then(|p| p.result.as_ref()) {
        None => pane_status::loading_line(ui, t.tr("journal.preview.loading")),
        Some(Err(e)) => {
            ui.label(RichText::new(crate::ui::common::capitalize(trellis_core::error::plain_message(e).trim())).color(color::weak()));
        }
        Some(Ok(p)) => {
            let tone = match &row.subject {
                Subject::Head => color::accent(),
                Subject::Branch(b) => line(snap.branch_swatch(b)),
            };
            lane(ui, st, tone, &p.history);
        }
    }
    ui.add_space(12.0);
    let why = journal_restore::plan(snap, row, st.job.is_some(), t).err();
    ui.horizontal(|ui| {
        if ui.add(buttons::primary(t.tr("journal.restore.button")).enabled(why.is_none()).why(why.as_deref())).clicked() {
            msgs.push(Msg::Journal(JournalMsg::Restore(row.key())));
        }
        if ui.add(buttons::secondary(t.tr("journal.create_branch"))).clicked() {
            msgs.push(Msg::Journal(JournalMsg::CreateBranch(row.key())));
        }
    });
    reason::line(ui, why.as_deref(), Align::Min);
    if !row.message.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(t.tr("journal.preview.raw")).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
        let raw = RichText::new(trellis_core::text::printable(&row.message)).font(type_scale::mono(type_scale::SMALL)).color(color::weak());
        ui.add(egui::Label::new(raw).wrap());
    }
}

/// The commits as a vertical lane: a dot per commit joined by a 2 px line in `tone`, with the
/// summary, short SHA and age beside each.
pub(super) fn lane(ui: &mut Ui, st: &AppState, tone: Color32, history: &[CommitLine]) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, LANE_ROW_H * history.len() as f32), Sense::hover());
    let p = ui.painter();
    let x = rect.left() + 8.0;
    let y = |i: usize| rect.top() + LANE_ROW_H * (i as f32 + 0.5);
    if history.len() > 1 {
        p.line_segment([pos2(x, y(0)), pos2(x, y(history.len() - 1))], (2.0, tone));
    }
    let (body, small, mono) =
        (type_scale::font(type_scale::BODY), type_scale::font(type_scale::SMALL), type_scale::mono(type_scale::SMALL));
    let right = rect.right();
    for (i, c) in history.iter().enumerate() {
        let cy = y(i);
        p.circle_filled(pos2(x, cy), DOT_R, tone);
        let age = st.settings.format_time(c.time, st.view.now, st.view.utc_offset);
        let age_rect = p.text(pos2(right, cy), Align2::RIGHT_CENTER, age, small.clone(), color::weak());
        let sha_w = 64.0;
        let sha_x = age_rect.left() - 12.0 - sha_w;
        paint_cut_text(p, pos2(sha_x.max(x + 16.0), cy), &short(c.oid), mono.clone(), color::weak(), sha_w);
        paint_cut_text(p, pos2(x + 16.0, cy), &c.summary, body.clone(), color::text(), (sha_x - 8.0 - x - 16.0).max(0.0));
    }
}
