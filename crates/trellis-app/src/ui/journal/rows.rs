//! The journal's state list: 56 px painted rows, newest first, laid out only where visible (up
//! to 2,000 of them). Line 1 is the label and the time; line 2 the subject chip, "Now" on a
//! subject's current state, the short SHA and the commit's summary. Hovering shows git's raw
//! reflog text; right-click offers Restore…, Create branch here… and Copy > SHA.

use super::chip;
use crate::journal::JournalMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::{row_label, short, subject_name};
use crate::state::journal_restore;
use crate::ui::common::{menu_header, menu_item, menu_item_if};
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::{paint_cut_text, row_fill};
use eframe::egui::{self, Align2, CursorIcon, Rect, Sense, Ui, pos2, vec2};
use trellis_core::journal::Row;
use trellis_core::repo::Snapshot;

const ROW_H: f32 = 56.0;
const PAD_X: f32 = 16.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(view), Some(snap)) = (&st.journal, &st.snap) else { return };
    let rows = view.rows();
    // The capped footer takes the place of one more row.
    let n = rows.len() + usize::from(view.capped());
    egui::ScrollArea::vertical().id_salt("journal-rows").auto_shrink([false, false]).show_rows(ui, ROW_H, n, |ui, range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for i in range {
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
            let Some(row) = rows.get(i) else {
                let t = st.settings.lang();
                let font = type_scale::font(type_scale::SMALL);
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, crate::state::journal::capped_line(t), font, color::weak());
                continue;
            };
            let key = row.key();
            let selected = view.selected.as_ref() == Some(&key);
            let label = row_label(st.settings.lang(), row);
            // The row's accessible name: its label, as painted on its first line.
            crate::ui::access::name_button(&resp, true, &label);
            let time = ui.is_rect_visible(rect).then(|| {
                row_fill::paint(ui.painter(), rect, selected, resp.hovered());
                paint(ui, st, rect, row, &label)
            });
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            let over_time = time.zip(ui.ctx().pointer_hover_pos()).is_some_and(|(r, p)| r.contains(p));
            let resp = if resp.hovered() && over_time {
                resp.on_hover_text_at_pointer(st.settings.date_tooltip(row.at, st.view.now, st.view.utc_offset))
            } else if !row.message.is_empty() {
                resp.on_hover_text(trellis_core::text::printable(&row.message))
            } else {
                resp
            };
            if resp.clicked() {
                msgs.push(Msg::Journal(JournalMsg::Select(key.clone())));
            }
            resp.context_menu(|ui| menu(ui, st, snap, row, &label, msgs));
            ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
        }
    });
}

/// Paints one row's two lines; returns where the time went.
fn paint(ui: &Ui, st: &AppState, rect: Rect, row: &Row, label: &str) -> Rect {
    let t = st.settings.lang();
    let p = ui.painter();
    let (left, right) = (rect.left() + PAD_X, rect.right() - PAD_X);
    let (y1, y2) = (rect.top() + 18.0, rect.top() + 39.0);
    let small = type_scale::font(type_scale::SMALL);
    let when = st.settings.format_time(row.at, st.view.now, st.view.utc_offset);
    let time = p.text(pos2(right, y1), Align2::RIGHT_CENTER, when, small.clone(), color::weak());
    let body = type_scale::font(type_scale::BODY);
    paint_cut_text(p, pos2(left, y1), label, body, color::text(), (time.left() - 12.0 - left).max(0.0));

    let mut x = left;
    x += chip(p, x, y2, subject_name(&row.subject), (right - x) * 0.4) + 6.0;
    if row.current {
        x += chip(p, x, y2, t.tr("journal.now"), right - x) + 6.0;
    }
    let mono = type_scale::mono(type_scale::SMALL);
    let sha = paint_cut_text(p, pos2(x, y2), &short(row.oid), mono, color::weak(), (right - x).max(0.0));
    x += sha.rect.width() + 8.0;
    let summary = row.summary.as_deref().unwrap_or(t.tr("journal.gone"));
    paint_cut_text(p, pos2(x, y2), summary, small, color::weak(), (right - x).max(0.0));
    time
}

/// The row menu: its label as the header, Restore… (disabled with why), Create branch here…
/// and Copy > SHA.
fn menu(ui: &mut Ui, st: &AppState, snap: &Snapshot, row: &Row, label: &str, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    menu_header(ui, label);
    let why = journal_restore::plan(snap, row, st.job.is_some(), t).err();
    menu_item_if(msgs, ui, t.tr("journal.restore.button"), why.as_deref(), Msg::Journal(JournalMsg::Restore(row.key())));
    menu_item(msgs, ui, t.tr("journal.create_branch"), Msg::Journal(JournalMsg::CreateBranch(row.key())));
    ui.separator();
    ui.menu_button(t.tr("commits.menu.copy"), |ui| {
        menu_item(msgs, ui, t.tr("commits.menu.copy_sha"), Msg::Copy(row.oid.to_string()));
    });
}
