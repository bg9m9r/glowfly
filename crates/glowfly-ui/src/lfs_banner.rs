//! The banner under the toolbar when the active repository stores files in Git LFS but git-lfs
//! isn't installed (with the install help) or is older than Glowfly works with (with how to
//! update it), and the Git LFS page. Each form is dismissed per repository until Glowfly
//! restarts (state/lfs_notes.rs, update/lfs.rs).

use crate::theme::color;
use crate::widgets::Icon;
use crate::widgets::banner::{self, Banner, BannerKind};
use crate::widgets::buttons;
use eframe::egui;
use glowfly_model::lfs::LfsMsg;
use glowfly_model::msg::{Msg, PrefsMsg};
use glowfly_model::state::lfs::{INSTALL_URL, LfsProblem};
use glowfly_model::state::lfs_notes::{banner, banner_text};
use glowfly_model::state::{AppState, PrefPage};

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(b) = banner(st) else { return };
    let t = st.settings.lang();
    let strip = Banner::new(BannerKind::Warning).icon(Icon::Storage, color::warn()).text(banner_text(t, &st.lfs.install));
    banner::top(ctx, "lfs_banner", strip, |ui| {
        if ui.add(buttons::secondary(t.tr("common.dismiss.button"))).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::DismissBanner(b.workdir.to_path_buf())));
        }
        if ui.add(buttons::secondary(t.tr("lfs.banner.settings.button"))).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)));
        }
        let open = match b.problem {
            LfsProblem::Missing => ui.add(buttons::secondary(t.tr("lfs.page.get.button"))).on_hover_text(INSTALL_URL),
            LfsProblem::TooOld => {
                let tip = t.trf("lfs.update.tip", &[("url", &INSTALL_URL)]);
                ui.add(buttons::secondary(t.tr("lfs.update.button")).tip(&tip))
            }
        };
        if open.clicked() {
            msgs.push(Msg::OpenUrl(INSTALL_URL.into()));
        }
    });
}
