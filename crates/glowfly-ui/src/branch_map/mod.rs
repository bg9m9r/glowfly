//! The Branch map, shown in place of the graph: a 56 px header (title, subtitle, Close), then the
//! branches as a list on the left (`rows.rs`) and the selected branch's shared files on the right
//! (`detail.rs`). Below 720 px of width the pane stacks under the list. Loading, failure and the
//! empty states fill the body instead. The list is the plain form of the map; drawing it as a
//! picture comes later.

mod detail;
mod rows;

use crate::common::menu_item;
use crate::empty_state::{EmptyIcon, empty_state};
use crate::pane_status::{self, Failure};
use crate::theme::{color, sizes, type_scale};
use crate::widgets::buttons;
use eframe::egui::{Align, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_model::branch_map::BranchMapMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::state::branch_map::{self as words, BranchMapView};

const INSET: f32 = 16.0;
/// Narrower than this, the pane goes under the list.
const STACK_BELOW: f32 = 720.0;
const LIST_SHARE: f32 = 0.5;
const LIST_MIN_W: f32 = 360.0;

pub fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.session.branch_map.view else { return };
    let t = st.settings.lang();
    header(ui, t, view, msgs);
    body(ui, st, view, msgs);
}

fn body(ui: &mut Ui, st: &AppState, view: &BranchMapView, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    match (&view.plan, &view.result) {
        (Err(why), _) => {
            let main = st.snap.as_ref().and_then(|s| s.mainline());
            let (title, hint) = words::empty_words(t, *why, main.as_deref());
            empty_state(ui, EmptyIcon::Commits, title, &hint, &[]);
        }
        (_, None) => pane_status::loading(ui, t.tr("branch_map.loading")),
        // Trying again after a failure shows the spinner until the new answer arrives.
        (_, Some(Err(_))) if view.loading => pane_status::loading(ui, t.tr("branch_map.loading")),
        (_, Some(Err(e))) => {
            let retry = Some(Msg::BranchMap(BranchMapMsg::Retry));
            let error = words::failure(t, e);
            pane_status::failed(ui, t, Failure { title: t.tr("branch_map.failed"), error: &error, retry }, msgs);
        }
        (_, Some(Ok(_))) => split(ui, |ui| rows::show(ui, st, msgs), |ui| detail::show(ui, st)),
    }
}

/// The title and subtitle on the left, Close on the right.
fn header(ui: &mut Ui, t: Lang, view: &BranchMapView, msgs: &mut Vec<Msg>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), sizes::TWO_LINE_ROW_H), Sense::hover());
    let inner = rect.shrink2(vec2(INSET, 0.0));
    let mut row = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)));
    if row.add(buttons::secondary(t.tr("common.close.button"))).on_hover_text(t.tr("branch_map.close.tip")).clicked() {
        msgs.push(Msg::BranchMap(BranchMapMsg::Close));
    }
    let room = row.available_width() - 16.0;
    let left_rect = Rect::from_min_size(inner.min, vec2(room, sizes::TWO_LINE_ROW_H));
    let mut left = row.new_child(UiBuilder::new().max_rect(left_rect).layout(Layout::top_down(Align::Min)));
    left.add_space(9.0);
    left.spacing_mut().item_spacing.y = 2.0;
    let title = RichText::new(t.tr("branch_map.title")).font(type_scale::strong(&left, type_scale::SUBHEAD)).color(color::text());
    crate::widgets::truncate_label(&mut left, title, room);
    let sub = RichText::new(words::subtitle(t, view)).font(type_scale::font(type_scale::SMALL)).color(color::weak());
    crate::widgets::truncate_label(&mut left, sub, room);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
}

/// The list and the pane, side by side or stacked, with a 1 px divider between them.
fn split(ui: &mut Ui, list_ui: impl FnOnce(&mut Ui), pane_ui: impl FnOnce(&mut Ui)) {
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
    let mut left = ui.new_child(UiBuilder::new().max_rect(list).id_salt("branch-map-list"));
    left.set_clip_rect(list.intersect(ui.clip_rect()));
    list_ui(&mut left);
    ui.painter().line_segment(line, (1.0, color::border()));
    let mut right = ui.new_child(UiBuilder::new().max_rect(pane).id_salt("branch-map-detail"));
    right.set_clip_rect(pane.intersect(ui.clip_rect()));
    pane_ui(&mut right);
}

/// The LOCAL section menu's "Branch map" item.
pub(crate) fn open_item(ui: &mut Ui, t: Lang, msgs: &mut Vec<Msg>) {
    menu_item(msgs, ui, t.tr("branch_map.open.item"), Msg::BranchMap(BranchMapMsg::Open));
}
