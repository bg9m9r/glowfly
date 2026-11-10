//! What a diff with a Git LFS pointer on either side shows instead of the pointer text: the
//! sizes and what changed (`state::lfs_notes::summary_text`), with a way to the pointer text and
//! back; and the box it sits in, which the gitlink card shares (`card_frame`).

use crate::theme::color;
use crate::theme::type_scale;
use eframe::egui::{self, RichText, Ui};
use glowfly_model::lfs::LfsMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::lfs_notes::{LfsSummary, POINTER_BAR, SHOW_SUMMARY, SUMMARY_SHOW_POINTER};

/// The summary block in the diff's place.
pub(super) fn summary(ui: &mut Ui, s: &LfsSummary, msgs: &mut Vec<Msg>) {
    ui.add_space(6.0);
    card_frame(ui).show(ui, |ui| {
        ui.label(RichText::new(&s.title).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        for (line, oid) in &s.lines {
            let r = ui.add(egui::Label::new(RichText::new(line).color(color::weak())).wrap());
            if let Some(oid) = oid {
                r.on_hover_text(oid);
            }
        }
        if let Some(w) = &s.warning {
            ui.add(egui::Label::new(RichText::new(w).color(color::warn())).wrap());
        }
        ui.add_space(4.0);
        if ui.button(SUMMARY_SHOW_POINTER).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::ShowPointer(true)));
        }
    });
}

/// The box a diff shows in place of its rows: this summary's, and the gitlink card's, so the two
/// read as one family.
pub(super) fn card_frame(ui: &Ui) -> egui::Frame {
    let frame = egui::Frame::group(ui.style()).fill(color::banner()).inner_margin(egui::Margin::same(10));
    frame.outer_margin(egui::Margin::symmetric(8, 0))
}

/// The slim bar above the pointer text's rows, back to the summary.
pub(super) fn pointer_bar(ui: &mut Ui, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(POINTER_BAR).color(color::weak()));
        if ui.small_button(SHOW_SUMMARY).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::ShowPointer(false)));
        }
    });
}
