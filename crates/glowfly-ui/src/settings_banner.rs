//! The banner under the toolbar (and above Preferences) while Glowfly's settings file couldn't
//! be read: defaults are in use and nothing is saved until it can (update/settings_file.rs).
//! It stays until a Try again reads the file; Open settings folder shows where it is.

use crate::msg::{Msg, PrefsMsg};
use crate::state::AppState;
use crate::ui::theme::color;
use crate::ui::widgets::banner::{self, Banner, BannerKind};
use crate::ui::widgets::buttons;
use eframe::egui;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(u) = st.settings_file.unreadable.as_ref() else { return };
    let t = st.settings.lang();
    let b = Banner::new(BannerKind::Error)
        .title(t.tr("settings_file.banner.title"))
        .text(t.tr("settings_file.banner.text"))
        .detail(u.detail.as_str(), color::weak());
    banner::top(ctx, "settings_banner", b, |ui| {
        // Added right to left: Try again nearest the text.
        let tip = t.tr("settings_file.banner.folder.tip");
        if ui.add(buttons::secondary(t.tr("settings_file.banner.folder.button")).tip(tip)).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenSettingsFolder));
        }
        let why = u.reading.then(|| t.tr("settings_file.banner.reading"));
        let retry =
            buttons::primary(t.tr("common.try_again.button")).tip(t.tr("settings_file.banner.retry.tip")).enabled(!u.reading).why(why);
        if ui.add(retry).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::RetrySettingsRead));
        }
    });
}
