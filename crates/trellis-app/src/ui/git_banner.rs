//! The banner under the toolbar while git is missing or too old for some features (see
//! `trellis_core::git_version`), with a way to choose another git (Preferences > Git). It can be
//! closed until Trellis restarts or another git is chosen.

use crate::git_env::GitEnvMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::theme::color;
use eframe::egui::{self, RichText};

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(text) = st.git.banner() else { return };
    egui::TopBottomPanel::top("git_banner")
        .frame(egui::Frame::new().fill(color::banner()).inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Dismiss").clicked() {
                        msgs.push(Msg::GitEnv(GitEnvMsg::DismissBanner));
                    }
                    if ui.button("Choose git…").clicked() {
                        msgs.push(Msg::GitEnv(GitEnvMsg::ShowPage));
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(text).color(color::text())).wrap());
                    });
                });
            });
        });
}
