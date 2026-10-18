//! Help: the shortcut sheet, the palette, the welcome and the tour, the logs, the project's links,
//! the release notes and Check for updates, and About.

use super::items::item;
use crate::keymap::{BindingId as I, KeyAction};
use crate::logs::{LogKind, LogsMsg};
use crate::msg::{AppMsg, Msg, PaletteMsg, UpdateMsg};
use crate::onboarding::OnboardingMsg;
use crate::state::AppState;
use crate::state::menu_bar::{ISSUES, WEBSITE};
use eframe::egui;

pub(super) fn menu(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.menu_button(t.tr("menu.help.title"), |ui| {
        ui.set_min_width(240.0);
        let keys = &st.keymap;
        item(ui, keys, t.tr("menu.help.shortcuts"), Some(I::Sheet), None, Msg::Shortcut(KeyAction::ToggleSheet), msgs);
        item(ui, keys, t.tr("menu.help.palette"), Some(I::Palette), None, Msg::Palette(PaletteMsg::TogglePalette), msgs);
        ui.separator();
        item(ui, keys, t.tr("menu.help.welcome"), None, None, Msg::Onboarding(OnboardingMsg::ShowWelcome), msgs);
        item(ui, keys, t.tr("menu.help.tour"), None, None, Msg::Onboarding(OnboardingMsg::StartTour), msgs);
        ui.separator();
        let logs = [
            (t.tr("menu.help.activity_log"), LogKind::Activity),
            (t.tr("menu.help.error_log"), LogKind::Errors),
            (t.tr("menu.help.performance_log"), LogKind::Performance),
        ];
        for (label, kind) in logs {
            item(ui, keys, label, None, None, Msg::Logs(LogsMsg::Show(kind)), msgs);
        }
        item(ui, keys, t.tr("menu.help.logs_folder"), None, None, Msg::Logs(LogsMsg::OpenFolder), msgs);
        ui.separator();
        item(ui, keys, t.tr("menu.help.website"), None, None, Msg::OpenUrl(WEBSITE.to_owned()), msgs);
        item(ui, keys, t.tr("menu.help.report_issue"), None, None, Msg::OpenUrl(ISSUES.to_owned()), msgs);
        ui.separator();
        item(ui, keys, t.tr("menu.help.release_notes"), None, None, Msg::Updates(UpdateMsg::ShowNotes), msgs);
        let checking = st.updates.busy().then_some(t.tr("menu.help.busy"));
        item(ui, keys, t.tr("menu.help.check_updates"), None, checking, Msg::Updates(UpdateMsg::CheckNow), msgs);
        item(ui, keys, t.tr("menu.help.about"), None, None, Msg::App(AppMsg::ShowAbout), msgs);
    });
}
