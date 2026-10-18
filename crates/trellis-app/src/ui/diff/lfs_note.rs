//! What a diff with a Git LFS pointer on either side shows instead of the pointer text: the
//! sizes and what changed (`state::lfs_notes::summary_text`), with a way to the pointer text and
//! back.

use crate::lfs::LfsMsg;
use crate::msg::Msg;
use crate::state::lfs_notes::{LfsSummary, POINTER_BAR, SHOW_SUMMARY, SUMMARY_SHOW_POINTER};
use crate::ui::theme::color;
use eframe::egui::{self, RichText, Ui};

/// The summary block in the diff's place.
pub(super) fn summary(ui: &mut Ui, s: &LfsSummary, msgs: &mut Vec<Msg>) {
    ui.add_space(6.0);
    let frame = egui::Frame::group(ui.style()).fill(color::banner()).inner_margin(egui::Margin::same(10));
    frame.outer_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
        ui.label(RichText::new(&s.title).strong().color(color::text()));
        for (line, oid) in &s.lines {
            let r = ui.add(egui::Label::new(RichText::new(line).color(color::weak())).wrap());
            if let Some(oid) = oid {
                r.on_hover_text(oid);
            }
        }
        if let Some(w) = &s.warning {
            ui.add(egui::Label::new(RichText::new(w).color(color::amber())).wrap());
        }
        ui.add_space(4.0);
        if ui.button(SUMMARY_SHOW_POINTER).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::ShowPointer(true)));
        }
    });
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
