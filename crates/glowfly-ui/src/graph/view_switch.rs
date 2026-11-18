//! The history view's bar: the Commits | Chapters switch above the graph (or the chapters). The
//! choice is saved per repository (`ChaptersMsg::SetMode`). While the search bar is open the
//! graph is shown whatever the choice, because search works on the commit list: the switch then
//! shows Commits and disables Chapters with the reason.

use super::layout::EDGE_INSET;
use crate::theme::color;
use crate::theme::sizes::ControlSize;
use crate::widgets::segmented::{Segment, segmented_opts};
use eframe::egui::{self, Align, Layout, Rect, Stroke, UiBuilder, vec2};
use glowfly_core::settings::HistoryMode;
use glowfly_model::msg::{ChaptersMsg, Msg};
use glowfly_model::state::AppState;

/// The bar's height: the switch's with 2 px above and below.
pub(crate) const BAR_H: f32 = ControlSize::Regular.metrics().h + 4.0;

/// Draws the bar: `searching` while the search bar is open, `chapters` while the chapters list
/// is what is shown below (it ends the bar with a line; the graph's header draws its own).
pub(super) fn show(ui: &mut egui::Ui, st: &AppState, searching: bool, chapters: bool, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    // Advanced by hand: the bar is exactly `BAR_H` tall, with no item spacing after it.
    let rect = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), BAR_H));
    ui.add_space(BAR_H);
    let mut inner =
        ui.new_child(UiBuilder::new().max_rect(rect.shrink2(vec2(EDGE_INSET, 0.0))).layout(Layout::left_to_right(Align::Center)));
    let current = if searching { HistoryMode::Commits } else { st.session.chapters.mode };
    let why = searching.then(|| t.tr("chapters.switch.search.why"));
    let options = [
        Segment::new(HistoryMode::Commits, t.tr("chapters.switch.commits.label"), t.tr("chapters.switch.commits.tip")),
        Segment::new(HistoryMode::Chapters, t.tr("chapters.switch.chapters.label"), t.tr("chapters.switch.chapters.tip")).why(why),
    ];
    if let Some(mode) = segmented_opts(&mut inner, ControlSize::Regular, current, &options) {
        msgs.push(Msg::Chapters(ChaptersMsg::SetMode(mode)));
    }
    if chapters {
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, color::border()));
    }
}
