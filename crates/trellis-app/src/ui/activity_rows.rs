//! The activity tab of the logs panel: one 32 px row per run or note (result mark, title, weak
//! summary, duration and relative time on the right, a chevron when it folds), routine
//! auto-fetches folded into one grey row, and earlier sessions under their headers, drawn
//! dimmer. The rows come from `state::activity_rows::rows`; the open part is in
//! ui/activity_fold.rs. Newest at the bottom, where the scroll sticks.

use super::theme::{color, type_scale};
use super::widgets::severity::{self, Severity};
use super::widgets::{self, Icon, paint_cut_text, paint_icon_sized, row_fill};
use crate::logs::LogsMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::activity_rows::{ActivityRow, SessionHead, foldable, rows, title_of};
use crate::state::logs::EntryId;
use eframe::egui::{self, Align, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use std::time::Duration;
use trellis_core::dates;
use trellis_core::i18n::Lang;
use trellis_core::logs::format_took;

pub(super) const ROW_H: f32 = 32.0;
/// A routine group's sub-rows.
const SUB_ROW_H: f32 = 28.0;
const HEAD_H: f32 = 30.0;
/// Room inside a row before its mark, and at its right end.
const PAD: f32 = 8.0;
/// The result mark's column and size.
const MARK_W: f32 = 16.0;
const MARK: f32 = widgets::icon::SIZE_SM;
/// Where a row's title starts, and its open part.
pub(super) const TITLE_X: f32 = PAD + MARK_W + PAD;
/// Between the title, the summary and the right-hand columns.
const GAP: f32 = 8.0;
/// How much an earlier session's marks are faded.
const DIM: f32 = 0.6;

pub(super) fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    if st.log.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(t.tr("logs.activity.empty")).color(color::faint()));
        return;
    }
    // A reveal lets go of the bottom for its frame and jumps there within it: sticking, or an
    // animation that only starts next frame, would be pulled back to the bottom.
    let revealing = st.view.activity_reveal.is_some();
    let area = egui::ScrollArea::vertical().stick_to_bottom(!revealing).animated(!revealing);
    area.auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (n, row) in rows(&st.log).into_iter().enumerate() {
            let (rect, id) = match row {
                ActivityRow::Session(head) => {
                    session_head(ui, st, head, n == 0);
                    continue;
                }
                ActivityRow::Entry(i) => (entry(ui, st, i, msgs), st.log[i].id),
                ActivityRow::Routine { first, last, count } => (routine(ui, st, first, last, count, msgs), st.log[first].id),
            };
            if st.view.activity_reveal == Some(id) {
                // The row's own line at the top, its open part below it.
                ui.scroll_to_rect(rect, Some(Align::Min));
                st.view.activity_reveal = None;
            }
        }
        // Once: a row that is gone (cleared) isn't looked for again.
        st.view.activity_reveal = None;
    });
}

/// "Earlier session · <date>" or "This session", with a hairline under it.
fn session_head(ui: &mut Ui, st: &AppState, head: SessionHead, first: bool) {
    let t = st.settings.lang();
    if !first {
        ui.add_space(8.0);
    }
    let text = match head {
        SessionHead::Earlier(at) => {
            let date = st.settings.format_verbose(at, st.view.utc_offset);
            t.trf("logs.activity.earlier_session", &[("date", &date)])
        }
        SessionHead::This => t.tr("logs.activity.this_session").to_owned(),
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), HEAD_H), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
    let p = ui.painter();
    let font = type_scale::section(ui);
    paint_cut_text(p, pos2(rect.left() + PAD, rect.center().y), &text, font, color::weak(), rect.width() - 2.0 * PAD);
    p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0_f32, color::border()));
}

/// What a row shows, painted by `paint_row`.
pub(super) struct RowLook<'a> {
    pub title: &'a str,
    pub title_color: Color32,
    pub summary: &'a str,
    pub took_ms: Option<u64>,
    /// When it ended (Unix seconds), and the tooltip over the time.
    pub at: i64,
    pub time_tip: String,
    /// None: it can't fold.
    pub open: Option<bool>,
    /// A routine group's sub-row: 28 px of small faint text, its time as the title (with the
    /// tooltip), nothing on the right but the duration.
    pub sub: bool,
}

/// Allocates and paints one row; `mark` paints its result mark centred on the point given.
/// Returns its response, which senses clicks when it folds.
pub(super) fn paint_row(ui: &mut Ui, st: &AppState, look: &RowLook, mark: impl FnOnce(&egui::Painter, Pos2)) -> Response {
    let t = st.settings.lang();
    let sense = if look.open.is_some() { Sense::click() } else { Sense::hover() };
    let height = if look.sub { SUB_ROW_H } else { ROW_H };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), sense);
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let p = ui.painter();
    row_fill::paint(p, rect, false, resp.hovered());
    let cy = rect.center().y;
    mark(p, pos2(rect.left() + PAD + MARK_W / 2.0, cy));
    let small = type_scale::font(type_scale::SMALL);
    let mut right = rect.right() - PAD;
    if let Some(open) = look.open {
        widgets::chevron(p, pos2(right - 5.0, cy), open, color::weak());
        right -= 12.0 + GAP;
    }
    let mut time_rect = Rect::NOTHING;
    if !look.sub {
        let time = dates::relative(t, st.view.now - look.at);
        time_rect = right_text(p, &mut right, cy, &time, &small);
    }
    if let Some(ms) = look.took_ms {
        right_text(p, &mut right, cy, &format_took(Duration::from_millis(ms)), &small);
    }
    let left = rect.left() + TITLE_X;
    let (font, summary_color) = if look.sub { (small, color::faint()) } else { (type_scale::font(type_scale::BODY), color::weak()) };
    let title = paint_cut_text(p, pos2(left, cy), look.title, font.clone(), look.title_color, right - left);
    if look.sub {
        time_rect = title.rect;
    }
    let from = title.rect.right() + GAP;
    let summary = paint_cut_text(p, pos2(from, cy), look.summary, font, summary_color, right - from);
    let pointer = ui.ctx().pointer_hover_pos();
    if pointer.is_some_and(|q| time_rect.contains(q)) {
        return resp.on_hover_text(look.time_tip.as_str());
    }
    if summary.hovered(pointer) {
        return resp.on_hover_text(look.summary);
    }
    if title.hovered(pointer) {
        return resp.on_hover_text(look.title);
    }
    resp
}

/// Paints `text` right-aligned at `right` and moves `right` past it and a gap; returns where.
fn right_text(p: &egui::Painter, right: &mut f32, cy: f32, text: &str, font: &FontId) -> Rect {
    let r = p.text(pos2(*right, cy), Align2::RIGHT_CENTER, text, font.clone(), color::weak());
    *right = r.left() - GAP - 4.0;
    r
}

/// The full date and time of `at`, for tooltips.
fn verbose(st: &AppState, at: i64) -> String {
    st.settings.format_verbose(at, st.view.utc_offset)
}

/// The words a screen reader hears for a row: "<title>, <done|failed>, <summary>".
fn spoken(t: Lang, title: &str, ok: bool, summary: &str) -> String {
    let status = if ok { t.tr("logs.activity.done") } else { t.tr("logs.activity.failed") };
    if summary.is_empty() { format!("{title}, {status}") } else { format!("{title}, {status}, {summary}") }
}

/// A run or a note, and its open part when it is folded open. Returns its row's rect.
fn entry(ui: &mut Ui, st: &AppState, i: usize, msgs: &mut Vec<Msg>) -> Rect {
    let e = &st.log[i];
    let t = st.settings.lang();
    let dim = e.earlier.is_some();
    let open = foldable(e).then(|| st.logs.open.contains(&e.id));
    let summary = e.run.as_ref().map_or("", |r| r.summary.as_str());
    let look = RowLook {
        title: title_of(e),
        title_color: if dim { color::weak() } else { color::text() },
        summary,
        took_ms: e.run.as_ref().and_then(|r| r.took_ms),
        at: e.at,
        time_tip: verbose(st, e.at),
        open,
        sub: false,
    };
    let fade = |c: Color32| if dim { c.gamma_multiply(DIM) } else { c };
    let (ok, is_run) = (e.ok, e.run.is_some());
    let resp = paint_row(ui, st, &look, |p, c| match (ok, is_run) {
        (true, true) => paint_icon_sized(p, c, Icon::Check, fade(color::added()), MARK),
        (true, false) => {
            p.circle_filled(c, 2.5, fade(color::faint()));
        }
        (false, _) => severity::paint(p, c, MARK, Severity::Error, fade(color::removed())),
    });
    let name = spoken(t, look.title, ok, summary);
    let kind = if open.is_some() { WidgetType::Button } else { WidgetType::Label };
    resp.widget_info(|| WidgetInfo::labeled(kind, true, &name));
    if resp.clicked() {
        msgs.push(Msg::Logs(LogsMsg::Fold(e.id)));
    }
    if open == Some(true) {
        super::activity_fold::show(ui, st, e, msgs);
    }
    resp.rect
}

/// Routine successes `first..=last`, standing for `count` runs: one grey row, and the runs it
/// holds as sub-rows when it is folded open. Returns the grey row's rect.
fn routine(ui: &mut Ui, st: &AppState, first: usize, last: usize, count: u32, msgs: &mut Vec<Msg>) -> Rect {
    let t = st.settings.lang();
    let (head, newest) = (&st.log[first], &st.log[last]);
    let id: EntryId = head.id;
    let open = st.logs.open.contains(&id);
    let summary = if count == 1 {
        t.tr("logs.activity.routine.count.one").to_owned()
    } else {
        t.trn("logs.activity.routine.count", count as usize, &[])
    };
    let (from, to) = (verbose(st, head.at), verbose(st, newest.at));
    let title = t.tr("logs.activity.routine.title");
    let look = RowLook {
        title,
        title_color: color::weak(),
        summary: &summary,
        took_ms: None,
        at: newest.at,
        time_tip: t.trf("logs.activity.routine.tip", &[("first", &from), ("last", &to)]),
        open: Some(open),
        sub: false,
    };
    let dim = head.earlier.is_some();
    let resp = paint_row(ui, st, &look, |p, c| {
        let col = if dim { color::faint().gamma_multiply(DIM) } else { color::faint() };
        paint_icon_sized(p, c, Icon::Refresh, col, MARK);
    });
    let name = spoken(t, title, true, &summary);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
    if resp.clicked() {
        msgs.push(Msg::Logs(LogsMsg::Fold(id)));
    }
    if open {
        for e in &st.log[first..=last] {
            super::activity_fold::sub_row(ui, st, e);
        }
    }
    resp.rect
}
