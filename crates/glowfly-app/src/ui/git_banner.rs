//! The banner under the toolbar while git is missing (an error) or too old for some features (a
//! warning; see `glowfly_core::git_version`), with a way to choose another git (Preferences >
//! Git). It can be closed until Glowfly restarts or another git is chosen.

use crate::git_env::GitEnvMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::widgets::banner::{self, Banner, BannerKind};
use crate::ui::widgets::buttons;
use eframe::egui;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(text) = st.git.banner() else { return };
    let t = st.settings.lang();
    let kind = if st.git.git_unusable() { BannerKind::Error } else { BannerKind::Warning };
    banner::top(ctx, "git_banner", Banner::new(kind).text(text), |ui| {
        if ui.add(buttons::secondary(t.tr("common.dismiss.button"))).clicked() {
            msgs.push(Msg::GitEnv(GitEnvMsg::DismissBanner));
        }
        if ui.add(buttons::secondary(t.tr("git_banner.choose.button"))).clicked() {
            msgs.push(Msg::GitEnv(GitEnvMsg::ShowPage));
        }
    });
}
