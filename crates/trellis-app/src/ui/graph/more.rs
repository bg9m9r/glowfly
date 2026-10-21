//! Loading more of a truncated history from the graph: the near-end check, and the row below
//! the last commit while more loads (or a filtering search looks further back) or after the
//! load failed.

use crate::msg::{LoadMsg, Msg};
use crate::state::AppState;
use crate::state::load_more::MoreRow;
use crate::state::refresh::can_load_more;
use crate::ui::pane_status::plain_error;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use crate::ui::widgets::severity::{self, Severity};
use crate::ui::widgets::truncate::paint_cut_text;
use eframe::egui::{self, Align, Align2, Layout, Rect, RichText, Sense, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::i18n::Lang;

/// How close to the last loaded row the graph's last visible row gets before more is loaded.
pub(super) const NEAR_END_ROWS: usize = 40;

/// The spinner's and the warning mark's size.
const MARK: f32 = 14.0;
/// From the mark's left edge to the title's.
const TEXT_GAP: f32 = 22.0;
/// The button's inset from the row's right edge.
const RIGHT_INSET: f32 = 16.0;
/// Between the title and the reason, and between the reason and the button side.
const SPACE: f32 = 12.0;

/// Whether the graph, showing rows up to `last_visible` (exclusive) of `total`, should ask for
/// more history. update checks the same conditions again; this keeps the view from asking every
/// frame while a load is pending.
pub(super) fn wants_more(st: &AppState, last_visible: usize, total: usize, filtered: bool) -> bool {
    st.settings.load_more_on_scroll
        && !filtered
        && st.snap.as_ref().is_some_and(can_load_more)
        && !st.loading_more()
        && last_visible + NEAR_END_ROWS >= total
}

/// The row below the last commit, starting at the message column's left edge `text_x`. It is
/// one `row_h` allocation, so the graph's rows stay uniform.
pub(super) fn row(ui: &mut egui::Ui, t: Lang, row_h: f32, text_x: f32, r: &MoreRow, msgs: &mut Vec<Msg>) {
    match r {
        MoreRow::Loading => spinner_row(ui, t.tr("graph.loading_more"), row_h, text_x),
        MoreRow::SearchingOlder => spinner_row(ui, t.tr("graph.searching_older"), row_h, text_x),
        MoreRow::Failed { error, retrying } => failed_row(ui, t, row_h, text_x, error, *retrying, msgs),
    }
}

/// A spinner and a weak note. It can't be clicked or selected.
fn spinner_row(ui: &mut egui::Ui, text: &str, row_h: f32, text_x: f32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
    let mid = rect.center().y;
    // Painted rather than added, so the row's layout stays one allocation.
    egui::Spinner::new()
        .size(MARK)
        .color(color::weak())
        .paint_at(ui, Rect::from_min_size(pos2(text_x, mid - MARK / 2.0), vec2(MARK, MARK)));
    ui.painter_at(rect).text(pos2(text_x + TEXT_GAP, mid), Align2::LEFT_CENTER, text, type_scale::font(type_scale::BODY), color::weak());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
}

/// The load failed: the warning mark, the title, the reason cut to the room left (whole on
/// hover), and Try again on the right, with a note while an automatic retry waits.
fn failed_row(ui: &mut egui::Ui, t: Lang, row_h: f32, text_x: f32, error: &str, retrying: bool, msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
    let title = t.tr("graph.load_more_failed.title");
    let reason = plain_error(error);
    let mid = rect.center().y;
    let painter = ui.painter_at(rect);
    severity::paint(&painter, pos2(text_x + MARK / 2.0, mid), MARK, Severity::Warning, color::warn());
    let title_font = type_scale::strong(ui, type_scale::BODY);
    let title_rect = painter.text(pos2(text_x + TEXT_GAP, mid), Align2::LEFT_CENTER, title, title_font, color::text());
    let reason_x = title_rect.right() + SPACE;

    // The button side right of the title, in a child so the parent's cursor moves by the one
    // allocation only. The note is cut short rather than run into the title.
    let right =
        Rect::from_min_max(pos2(reason_x.min(rect.right() - RIGHT_INSET), rect.top()), pos2(rect.right() - RIGHT_INSET, rect.bottom()));
    let mut side = ui.new_child(UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)));
    if side.add(buttons::secondary(t.tr("common.try_again.button")).small()).clicked() {
        msgs.push(Msg::Load(LoadMsg::RetryLoadMore));
    }
    if retrying {
        side.add(egui::Label::new(RichText::new(t.tr("graph.load_more_failed.retrying")).color(color::weak())).truncate());
    }
    let room = side.min_rect().left() - SPACE - reason_x;
    let cut = paint_cut_text(&painter, pos2(reason_x, mid), &reason, type_scale::font(type_scale::BODY), color::weak(), room);
    let resp = if cut.hovered(ui.ctx().pointer_hover_pos()) { resp.on_hover_text(&reason) } else { resp };
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, format!("{title}: {reason}")));
}
