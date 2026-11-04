//! Preferences > General > Notifications (desktop notifications, where notifications show,
//! behind alerts and the push and pull confirmations) and Commit messages (spell checking).

use super::{check_row, combo, help, row, section};
use crate::theme::controls;
use eframe::egui::Ui;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{Corner, Settings};
use glowfly_model::msg::{Msg, PrefsMsg};

/// The Notifications group.
pub fn notifications(ui: &mut Ui, t: Lang, s: &mut Settings, msgs: &mut Vec<Msg>) {
    section(ui, t.tr("prefs.general.section.notifications"));
    row(ui, t.tr("prefs.ui.desktop_notifications.label"), |ui| {
        // Not bound to the setting: update saves it and sends the confirming notification.
        let mut on = s.desktop_notifications;
        if controls::checkbox(ui, &mut on, t.tr("prefs.ui.desktop_notifications.check")).changed() {
            msgs.push(Msg::Prefs(PrefsMsg::SetDesktopNotifications(on)));
        }
        help(ui, t.tr("prefs.ui.desktop_notifications.help"));
    });
    row(ui, t.tr("prefs.ui.notifications.label"), |ui| {
        combo(
            ui,
            "toasts",
            &mut s.notification_location,
            &[
                (Corner::BottomLeft, t.tr("prefs.ui.corner.bottom_left")),
                (Corner::BottomRight, t.tr("prefs.ui.corner.bottom_right")),
                (Corner::TopLeft, t.tr("prefs.ui.corner.top_left")),
                (Corner::TopRight, t.tr("prefs.ui.corner.top_right")),
            ],
        )
    });
    let (label, check, text) =
        (t.tr("prefs.ui.behind_alerts.label"), t.tr("prefs.ui.behind_alerts.check"), t.tr("prefs.ui.behind_alerts.help"));
    check_row(ui, label, &mut s.behind_alerts, check, Some(text));
    let (label, check, text) =
        (t.tr("prefs.ui.confirm_push_pull.label"), t.tr("prefs.ui.confirm_push_pull.check"), t.tr("prefs.ui.confirm_push_pull.help"));
    check_row(ui, label, &mut s.confirm_push_pull, check, Some(text));
}

/// The Commit messages group.
pub fn commit_messages(ui: &mut Ui, t: Lang, s: &mut Settings) {
    section(ui, t.tr("prefs.general.section.commit_messages"));
    let (label, check, text) = (t.tr("prefs.ui.spellcheck.label"), t.tr("prefs.ui.spellcheck.check"), t.tr("prefs.ui.spellcheck.help"));
    check_row(ui, label, &mut s.spellcheck, check, Some(text));
}
