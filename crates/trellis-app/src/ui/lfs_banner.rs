//! The banner under the toolbar when the active repository stores files in Git LFS but git-lfs
//! isn't installed, with the install help and the Git LFS page. Dismissed per repository until
//! Trellis restarts (state/lfs_notes.rs, update/lfs.rs).

use crate::lfs::LfsMsg;
use crate::msg::{Msg, PrefsMsg};
use crate::state::lfs::INSTALL_URL;
use crate::state::lfs_notes::{BANNER_DISMISS, BANNER_GET, BANNER_SETTINGS, BANNER_TEXT, banner_workdir};
use crate::state::{AppState, PrefPage};
use crate::ui::theme::color;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, RichText};

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(workdir) = banner_workdir(st) else { return };
    egui::TopBottomPanel::top("lfs_banner")
        .frame(egui::Frame::new().fill(color::banner()).inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(BANNER_DISMISS).clicked() {
                        msgs.push(Msg::Lfs(LfsMsg::DismissBanner(workdir.to_path_buf())));
                    }
                    if ui.button(BANNER_SETTINGS).clicked() {
                        msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)));
                    }
                    if ui.button(BANNER_GET).on_hover_text(INSTALL_URL).clicked() {
                        msgs.push(Msg::OpenUrl(INSTALL_URL.into()));
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                        widgets::paint_icon(ui.painter(), rect.center(), Icon::Storage, color::amber());
                        ui.add(egui::Label::new(RichText::new(BANNER_TEXT).color(color::text())).wrap());
                    });
                });
            });
        });
}
