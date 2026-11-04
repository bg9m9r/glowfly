//! The status bar's What's new chip after an upgrade (`AppState::updates_news_chip`): a quiet
//! neutral Badge, "What's new in x.y.z", that opens the release notes, which takes it away. Like
//! the update chip, its click spans the bar's whole height and its text is cut with "…" when
//! the bar runs out of room (whole in the tooltip).

use super::theme::color;
use super::widgets::chip::{Chip, ChipSize, Look};
use eframe::egui::{self, Sense, vec2};
use glowfly_model::msg::{Msg, UpdateMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::release_notes::news_chip_text;

/// The chip's size: a Badge, as the update chip beside it.
const CHIP: ChipSize = ChipSize::Badge;

pub fn status_item(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(news) = st.updates_news_chip() else { return };
    let t = st.settings.lang();
    let text = news_chip_text(t, &news.version);
    let chip = |fill| Chip::new(CHIP, text.clone()).look(Look::new(fill, color::text()).edged(color::border()));
    let full_w = chip(color::raised()).width(ui.ctx());
    let w = full_w.min(ui.available_width());
    // The click takes the bar's whole height; the badge is painted at its own.
    let (hit, resp) = ui.allocate_exact_size(vec2(w, ui.available_height().max(CHIP.metrics().h)), Sense::click());
    let rect = super::widgets::chip::rect_at(CHIP, hit.left(), hit.center().y, w);
    let fill = if resp.hovered() { color::hover() } else { color::raised() };
    chip(fill).max_w(w).paint_in_rect(ui.painter(), rect);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
    let tip = t.tr("updates.whats_new.tip");
    let tip = if w < full_w { format!("{text}\n{tip}") } else { tip.to_owned() };
    if resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        msgs.push(Msg::Updates(UpdateMsg::ShowNotes));
    }
}
