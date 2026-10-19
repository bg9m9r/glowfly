//! The strip under the toolbar while the active repository has an operation in progress or
//! conflicts left (`state::op_strip`): the operation, how its conflicts stand, and Show work in
//! progress. A click anywhere on it opens the WIP panel; Continue and Abort stay in the panel.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::op_strip::strip_text;
use eframe::egui::{self, Align, CursorIcon, Layout, RichText, Sense, UiBuilder};

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = st.snap.as_ref().filter(|_| !st.dashboard.open) else { return };
    let t = st.settings.lang();
    let Some(text) = strip_text(t, snap, st.conflict_files.resolved_rows(snap).len()) else { return };
    egui::TopBottomPanel::top("op_strip").frame(egui::Frame::new().fill(color::banner())).show(ctx, |ui| {
        // Sensed below the button, so a click on the button stays the button's.
        let strip = ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(t.tr("op_strip.show.button")).clicked() {
                            msgs.push(Msg::Repo(RepoMsg::ShowInProgress));
                        }
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let (icon, tint) = if text.conflicts { (Icon::Warning, color::amber()) } else { (Icon::Check, color::green()) };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
                            widgets::paint_icon(ui.painter(), rect.center(), icon, tint);
                            ui.label(RichText::new(&text.title).strong());
                            if let Some((detail, all)) = &text.detail {
                                let tint = if *all { color::green() } else { color::amber() };
                                ui.add(egui::Label::new(RichText::new(detail).color(tint)).truncate());
                            }
                        });
                    });
                });
            });
        });
        let r = strip.response.on_hover_cursor(CursorIcon::PointingHand);
        if r.clicked() {
            msgs.push(Msg::Repo(RepoMsg::ShowInProgress));
        }
    });
}

#[cfg(test)]
#[path = "snapshot_tests_op_strip.rs"]
mod snapshot_tests;
