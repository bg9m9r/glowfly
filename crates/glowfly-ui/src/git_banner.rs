//! The banner under the toolbar while git is missing (an error) or too old for some features (a
//! warning; see `glowfly_core::git_version`), with a way to choose another git (Preferences >
//! Git). When the saved git can't run and the git on PATH works, it also offers to use that one
//! (checked first, as on the Preferences page). Remind me later hides it until Glowfly restarts
//! or another git is chosen. A too-old git also gets How to update git (the Git project's
//! download page for this OS) and Don't show again, which hides it for good while that version
//! runs; a git that can't run has neither, since commits fail without it.

use crate::git_env::GitEnvMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::widgets::banner::{self, Banner, BannerKind};
use crate::ui::widgets::buttons;
use eframe::egui;
use glowfly_core::git_version::{self, show as show_version};
use glowfly_core::native_path::shown;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(text) = st.git_banner() else { return };
    let t = st.settings.lang();
    let kind = if st.git.git_unusable() { BannerKind::Error } else { BannerKind::Warning };
    let old = st.git.too_old().filter(|_| !st.git.git_unusable());
    banner::top(ctx, "git_banner", Banner::new(kind).text(text), |ui| {
        // Added right to left: Don't show again rightmost, the offer nearest the text.
        if let Some(v) = old {
            let tip = t.trf("git_banner.never.tip", &[("version", &show_version(v))]);
            if ui.add(buttons::quiet(t.tr("git_banner.never.button")).tip(&tip)).clicked() {
                msgs.push(Msg::GitEnv(GitEnvMsg::HideBannerForGood));
            }
        }
        if ui.add(buttons::quiet(t.tr("git_banner.later.button")).tip(t.tr("git_banner.later.tip"))).clicked() {
            msgs.push(Msg::GitEnv(GitEnvMsg::DismissBanner));
        }
        if old.is_some() {
            let url = git_version::download_url();
            let tip = t.trf("git_banner.update.tip", &[("url", &url)]);
            if ui.add(buttons::secondary(t.tr("git_banner.update.button")).tip(&tip)).clicked() {
                msgs.push(Msg::OpenUrl(url.to_owned()));
            }
        }
        if ui.add(buttons::secondary(t.tr("git_banner.choose.button"))).clicked() {
            msgs.push(Msg::GitEnv(GitEnvMsg::ShowPage));
        }
        if let Some((path, v)) = st.git.path_offer() {
            let label = t.trf("git_banner.use_path.button", &[("version", &show_version(v))]);
            let tip = t.trf("git_banner.use_path.tip", &[("path", &shown(path))]);
            let busy = st.git.page.checking.is_some();
            let why = busy.then(|| t.tr("git_banner.use_path.checking"));
            if ui.add(buttons::primary(label).tip(&tip).enabled(!busy).why(why)).clicked() {
                msgs.push(Msg::GitEnv(GitEnvMsg::UsePathGit));
            }
        }
    });
}
