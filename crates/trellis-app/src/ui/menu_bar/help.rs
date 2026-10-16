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
    ui.menu_button("Help", |ui| {
        ui.set_min_width(240.0);
        let keys = &st.keymap;
        item(ui, keys, "Keyboard shortcuts", Some(I::Sheet), None, Msg::Shortcut(KeyAction::ToggleSheet), msgs);
        item(ui, keys, "Command palette", Some(I::Palette), None, Msg::Palette(PaletteMsg::TogglePalette), msgs);
        ui.separator();
        item(ui, keys, "Show the welcome", None, None, Msg::Onboarding(OnboardingMsg::ShowWelcome), msgs);
        item(ui, keys, "Tour of the main window", None, None, Msg::Onboarding(OnboardingMsg::StartTour), msgs);
        ui.separator();
        let logs = [("Activity log", LogKind::Activity), ("Error log", LogKind::Errors), ("Performance log", LogKind::Performance)];
        for (label, kind) in logs {
            item(ui, keys, label, None, None, Msg::Logs(LogsMsg::Show(kind)), msgs);
        }
        item(ui, keys, "Open logs folder", None, None, Msg::Logs(LogsMsg::OpenFolder), msgs);
        ui.separator();
        item(ui, keys, "Trellis website", None, None, Msg::OpenUrl(WEBSITE.to_owned()), msgs);
        item(ui, keys, "Report an issue", None, None, Msg::OpenUrl(ISSUES.to_owned()), msgs);
        ui.separator();
        item(ui, keys, "Release notes", None, None, Msg::Updates(UpdateMsg::ShowNotes), msgs);
        let checking = st.updates.busy().then_some("A check or download is already running");
        item(ui, keys, "Check for updates", None, checking, Msg::Updates(UpdateMsg::CheckNow), msgs);
        item(ui, keys, "About Trellis…", None, None, Msg::App(AppMsg::ShowAbout), msgs);
    });
}
