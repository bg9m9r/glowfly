//! The operation journal, shown in place of the graph: a 56 px header (title, subtitle, the
//! States | Lost commits control, Refresh, Close), then on the States tab the states newest
//! first on the left (`rows.rs`) and the selected state's preview with Restore on the right
//! (`preview.rs`), and on the Lost commits tab the lost commits (`lost.rs`) and the selected
//! one's preview (`lost_preview.rs`). Below 720 px of width the preview stacks under the list.
//! Loading, failure and an empty list fill the body instead.

mod lost;
mod lost_preview;
mod preview;
mod rows;

use super::empty_state::{EmptyIcon, empty_state};
use super::pane_status::{self, Failure};
use super::theme::{color, type_scale};
use super::widgets::Icon;
use super::widgets::buttons;
use super::widgets::ghost::{ghost, icon_tip};
use super::widgets::segmented::{Segment, segmented_opts};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal_lost::JournalTab;
use crate::state::tip::Tip;
use crate::ui::theme::tokens::radius;
use eframe::egui::{self, Align, Color32, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use glowfly_model::journal::JournalMsg;

const HEADER_H: f32 = 56.0;
const INSET: f32 = 16.0;
/// Narrower than this, the preview goes under the list.
const STACK_BELOW: f32 = 720.0;
const LIST_SHARE: f32 = 0.55;
const LIST_MIN_W: f32 = 360.0;

pub fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.session.journal.view else { return };
    let t = st.settings.lang();
    header(ui, st, msgs);
    if view.tab == JournalTab::Lost {
        return lost::show(ui, st, msgs);
    }
    match &view.timeline {
        None => pane_status::loading(ui, t.tr("journal.loading")),
        Some(Err(e)) => {
            let retry = Some(Msg::Journal(JournalMsg::Refresh));
            pane_status::failed(ui, t, Failure { title: t.tr("journal.failed"), error: e, retry }, msgs);
        }
        Some(Ok(tl)) if tl.rows.is_empty() => {
            empty_state(ui, EmptyIcon::Journal, t.tr("journal.empty.title"), t.tr("journal.empty.hint"), &[]);
        }
        Some(Ok(_)) => split(ui, msgs, |ui, msgs| rows::show(ui, st, msgs), |ui, msgs| preview::show(ui, st, msgs)),
    }
}

/// The title and subtitle on the left; Close, the Refresh icon and the tabs on the right.
fn header(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let tab = st.session.journal.view.as_ref().map(|v| v.tab).unwrap_or_default();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
    let inner = rect.shrink2(vec2(INSET, 0.0));
    let mut row = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)));
    let close = t.tr("common.close.button");
    if row.add(buttons::secondary(close)).on_hover_text(t.tr("journal.close.tip")).clicked() {
        msgs.push(Msg::Journal(JournalMsg::Close));
    }
    row.add_space(8.0);
    let refresh = t.tr("journal.refresh.tip");
    if icon_tip(ghost(&mut row, Some(Icon::Refresh), None, refresh, true), &st.keymap, t, Tip::new(refresh)).clicked() {
        msgs.push(Msg::Journal(JournalMsg::Refresh));
    }
    row.add_space(12.0);
    let options = [
        Segment::new(JournalTab::States, t.tr("journal.tab.states.label"), t.tr("journal.tab.states.tip")),
        Segment::new(JournalTab::Lost, t.tr("journal.tab.lost.label"), t.tr("journal.tab.lost.tip")),
    ];
    if let Some(tab) = segmented_opts(&mut row, tab, &options) {
        msgs.push(Msg::Journal(JournalMsg::Tab(tab)));
    }
    let room = row.available_width() - 16.0;
    let left_rect = Rect::from_min_size(inner.min, vec2(room, HEADER_H));
    let mut left = row.new_child(UiBuilder::new().max_rect(left_rect).layout(Layout::top_down(Align::Min)));
    left.add_space(9.0);
    left.spacing_mut().item_spacing.y = 2.0;
    let title = RichText::new(t.tr("journal.title")).font(type_scale::strong(&left, type_scale::SUBHEAD)).color(color::text());
    super::widgets::truncate_label(&mut left, title, room);
    let sub = if tab == JournalTab::Lost { t.tr("journal.lost.subtitle") } else { t.tr("journal.subtitle") };
    let sub = RichText::new(sub).font(type_scale::font(type_scale::SMALL)).color(color::weak());
    super::widgets::truncate_label(&mut left, sub, room);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
}

/// The list and the preview, side by side or stacked, with a 1 px divider between them.
fn split(ui: &mut Ui, msgs: &mut Vec<Msg>, list_ui: impl FnOnce(&mut Ui, &mut Vec<Msg>), pane_ui: impl FnOnce(&mut Ui, &mut Vec<Msg>)) {
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let (list, pane, line) = if area.width() < STACK_BELOW {
        let y = (area.top() + area.height() * LIST_SHARE).round();
        let list = Rect::from_min_max(area.min, pos2(area.right(), y));
        let pane = Rect::from_min_max(pos2(area.left(), y + 1.0), area.max);
        (list, pane, [pos2(area.left(), y + 0.5), pos2(area.right(), y + 0.5)])
    } else {
        let x = (area.left() + (area.width() * LIST_SHARE).max(LIST_MIN_W)).round();
        let list = Rect::from_min_max(area.min, pos2(x, area.bottom()));
        let pane = Rect::from_min_max(pos2(x + 1.0, area.top()), area.max);
        (list, pane, [pos2(x + 0.5, area.top()), pos2(x + 0.5, area.bottom())])
    };
    ui.painter().rect_filled(list, 0.0, color::panel());
    let mut left = ui.new_child(UiBuilder::new().max_rect(list).id_salt("journal-list"));
    left.set_clip_rect(list.intersect(ui.clip_rect()));
    list_ui(&mut left, msgs);
    ui.painter().line_segment(line, (1.0, color::border()));
    let mut right = ui.new_child(UiBuilder::new().max_rect(pane).id_salt("journal-preview"));
    right.set_clip_rect(pane.intersect(ui.clip_rect()));
    pane_ui(&mut right, msgs);
}

/// A small chip with `fg` text on the toolbar fill, painted at `x` on the line centred at `y`;
/// returns its rect.
fn chip(p: &egui::Painter, x: f32, y: f32, text: &str, max_w: f32, fg: Color32) -> Rect {
    const H: f32 = 18.0;
    const PAD: f32 = 6.0;
    let font = type_scale::font(type_scale::SMALL);
    let galley =
        super::widgets::truncate::cut_galley(p.ctx(), super::widgets::truncate::cut_job(text, font, fg), (max_w - 2.0 * PAD).max(0.0));
    let w = galley.size().x + 2.0 * PAD;
    let rect = Rect::from_min_size(pos2(x, y - H / 2.0), vec2(w, H));
    p.rect_filled(rect, egui::CornerRadius::same(radius::SM), color::toolbar());
    p.galley(pos2(x + PAD, y - galley.size().y / 2.0), galley, fg);
    rect
}
