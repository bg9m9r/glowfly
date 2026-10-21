//! The banner under the toolbar when the active repository stores files in Git LFS but git-lfs
//! isn't installed, with the install help and the Git LFS page. Dismissed per repository until
//! Trellis restarts (state/lfs_notes.rs, update/lfs.rs).

use crate::lfs::LfsMsg;
use crate::msg::{Msg, PrefsMsg};
use crate::state::lfs::INSTALL_URL;
use crate::state::lfs_notes::{BANNER_DISMISS, BANNER_GET, BANNER_SETTINGS, BANNER_TEXT, banner_workdir};
use crate::state::{AppState, PrefPage};
use crate::ui::theme::color;
use crate::ui::widgets::Icon;
use crate::ui::widgets::banner::{self, Banner, BannerKind};
use crate::ui::widgets::buttons;
use eframe::egui;

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
