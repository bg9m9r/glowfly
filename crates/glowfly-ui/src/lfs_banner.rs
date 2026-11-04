//! The banner under the toolbar when the active repository stores files in Git LFS but git-lfs
//! isn't installed, with the install help and the Git LFS page. Dismissed per repository until
//! Glowfly restarts (state/lfs_notes.rs, update/lfs.rs).

use crate::theme::color;
use crate::widgets::Icon;
use crate::widgets::banner::{self, Banner, BannerKind};
use crate::widgets::buttons;
use eframe::egui;
use glowfly_model::lfs::LfsMsg;
use glowfly_model::msg::{Msg, PrefsMsg};
use glowfly_model::state::lfs::INSTALL_URL;
use glowfly_model::state::lfs_notes::{BANNER_DISMISS, BANNER_GET, BANNER_SETTINGS, BANNER_TEXT, banner_workdir};
use glowfly_model::state::{AppState, PrefPage};

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(workdir) = banner_workdir(st) else { return };
    let b = Banner::new(BannerKind::Warning).icon(Icon::Storage, color::warn()).text(BANNER_TEXT);
    banner::top(ctx, "lfs_banner", b, |ui| {
        if ui.add(buttons::secondary(BANNER_DISMISS)).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::DismissBanner(workdir.to_path_buf())));
        }
        if ui.add(buttons::secondary(BANNER_SETTINGS)).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)));
        }
        if ui.add(buttons::secondary(BANNER_GET)).on_hover_text(INSTALL_URL).clicked() {
            msgs.push(Msg::OpenUrl(INSTALL_URL.into()));
        }
    });
}
