//! The Range-diff view, shown in place of the graph: a header (the title, a subtitle naming the
//! two versions, how the ranges were cut with the counts, and Close), then the commit pairs on
//! the left (`pairs.rs`) and the selected pair's interdiff or sentence on the right
//! (`interdiff.rs`). Below 720 px of width the pane stacks under the list. Loading, a refusal,
//! a failure (Try again) and an empty comparison fill the body instead.

mod interdiff;
mod pairs;

use super::empty_state::{EmptyIcon, empty_state};
use super::pane_status::{self, Failure};
use super::theme::{color, type_scale};
use super::widgets::buttons;
use crate::msg::Msg;
use crate::range_diff::RangeDiffMsg;
use crate::state::AppState;
use crate::state::range_diff::{Phase, RangeDiffView, base_line, counts_line, outcome_failure, subtitle};
use eframe::egui::{Align, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use trellis_core::i18n::Lang;

/// Three lines: the title, the two versions, and the cut with the counts.
const HEADER_H: f32 = 72.0;
const INSET: f32 = 16.0;
/// Narrower than this, the pane goes under the list.
const STACK_BELOW: f32 = 720.0;
const LIST_SHARE: f32 = 0.4;
const LIST_MIN_W: f32 = 320.0;

pub fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.range_diff else { return };
    let t = st.settings.lang();
    header(ui, st, view, msgs);
    match &view.phase {
        Phase::Loading => pane_status::loading(ui, t.tr("range_diff.loading")),
        Phase::Held => failed(ui, t, t.tr("range_diff.held"), msgs),
        Phase::Failed(e) => failed(ui, t, e, msgs),
        Phase::Ready(outcome) => match (outcome_failure(t, outcome), view.compared()) {
            (Some(why), _) => failed(ui, t, &why, msgs),
            (None, Some(c)) if c.pairs.is_empty() => {
                empty_state(ui, EmptyIcon::Commits, t.tr("range_diff.empty.title"), t.tr("range_diff.empty.hint"), &[]);
            }
            (None, _) => split(ui, msgs, |ui, msgs| pairs::show(ui, st, msgs), |ui, _| interdiff::show(ui, st)),
        },
    }
}

fn failed(ui: &mut Ui, t: Lang, error: &str, msgs: &mut Vec<Msg>) {
    let retry = Some(Msg::RangeDiff(RangeDiffMsg::Retry));
    pane_status::failed(ui, t, Failure { title: t.tr("range_diff.failed"), error, retry }, msgs);
}

/// The title and the two weak lines on the left; Close on the right.
fn header(ui: &mut Ui, st: &AppState, view: &RangeDiffView, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
    let inner = rect.shrink2(vec2(INSET, 0.0));
    let mut row = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)));
    if row.add(buttons::secondary(t.tr("common.close.button"))).on_hover_text(t.tr("range_diff.close.tip")).clicked() {
        msgs.push(Msg::RangeDiff(RangeDiffMsg::Close));
    }
    let room = row.available_width() - 16.0;
    let left_rect = Rect::from_min_size(inner.min, vec2(room, HEADER_H));
    let mut left = row.new_child(UiBuilder::new().max_rect(left_rect).layout(Layout::top_down(Align::Min)));
    left.add_space(10.0);
    left.spacing_mut().item_spacing.y = 2.0;
    let title = RichText::new(t.tr("range_diff.title")).font(type_scale::strong(&left, type_scale::SUBHEAD)).color(color::text());
    super::widgets::truncate_label(&mut left, title, room);
    let small = |text: String| RichText::new(text).font(type_scale::font(type_scale::SMALL)).color(color::weak());
    let lines = match view.compared() {
        Some(c) => {
            let sub = subtitle(t, &view.origin, c, &st.settings, st.view.now, st.view.utc_offset);
            vec![sub, format!("{} · {}", base_line(t, c), counts_line(t, c))]
        }
        None => view.origin.label.clone().into_iter().collect(),
    };
    for line in lines {
        super::widgets::truncate_label(&mut left, small(line), room);
    }
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
}

/// The list and the pane, side by side or stacked, with a 1 px divider between them.
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
    let mut left = ui.new_child(UiBuilder::new().max_rect(list).id_salt("range-diff-list"));
    left.set_clip_rect(list.intersect(ui.clip_rect()));
    list_ui(&mut left, msgs);
    ui.painter().line_segment(line, (1.0, color::border()));
    let mut right = ui.new_child(UiBuilder::new().max_rect(pane).id_salt("range-diff-pane"));
    right.set_clip_rect(pane.intersect(ui.clip_rect()));
    pane_ui(&mut right, msgs);
}
