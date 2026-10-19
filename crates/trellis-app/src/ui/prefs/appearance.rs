//! Preferences > UI customization.

use super::{check_row, combo, help, row, section};
use crate::msg::{Msg, PrefsMsg};
use crate::themes::ThemeState;
use crate::ui::theme::controls;
use eframe::egui::{self, Ui};
use trellis_core::i18n::Lang;
use trellis_core::settings::Settings;

pub fn ui_page(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, msgs: &mut Vec<Msg>) {
    use trellis_core::settings::Corner;
    let t = s.lang();
    super::theme_row::theme_row(ui, s, themes, msgs);
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
    row(ui, t.tr("prefs.ui.desktop_notifications.label"), |ui| {
        // Not bound to the setting: update saves it and sends the confirming notification.
        let mut on = s.desktop_notifications;
        if controls::checkbox(ui, &mut on, t.tr("prefs.ui.desktop_notifications.check")).changed() {
            msgs.push(Msg::Prefs(PrefsMsg::SetDesktopNotifications(on)));
        }
        help(ui, t.tr("prefs.ui.desktop_notifications.help"));
    });
    interface_section(ui, t, s);
    super::appearance_graph::graph_section(ui, t, s);
}

/// Interface: zoom, the panels, toolbar labels, reduced motion, spell checking and the graph's
/// extras.
fn interface_section(ui: &mut Ui, t: Lang, s: &mut Settings) {
    use trellis_core::settings::BranchVisibility as BV;
    section(ui, t.tr("prefs.ui.interface.section"));
    row(ui, t.tr("prefs.ui.zoom.label"), |ui| {
        egui::ComboBox::from_id_salt("zoom").selected_text(format!("{}%", s.zoom_level())).width(super::fit(ui, 230.0)).show_ui(ui, |ui| {
            for p in trellis_core::settings::ZOOM_STEPS {
                ui.selectable_value(&mut s.zoom_percent, p, format!("{p}%"));
            }
        });
    });
    check_row(ui, t.tr("prefs.ui.left_panel.label"), &mut s.show_left_panel, t.tr("prefs.ui.left_panel.check"), None);
    check_row(ui, t.tr("prefs.ui.details_panel.label"), &mut s.show_details_panel, t.tr("prefs.ui.details_panel.check"), None);
    let (label, check, text) = (t.tr("prefs.ui.toolbar.label"), t.tr("prefs.ui.toolbar.check"), t.tr("prefs.ui.toolbar.help"));
    check_row(ui, label, &mut s.show_toolbar, check, Some(text));
    let (label, check, text) = (t.tr("prefs.ui.menu_bar.label"), t.tr("prefs.ui.menu_bar.check"), t.tr("prefs.ui.menu_bar.help"));
    check_row(ui, label, &mut s.show_menu_bar, check, Some(text));
    check_row(ui, t.tr("prefs.ui.toolbar_labels.label"), &mut s.toolbar_labels, t.tr("prefs.ui.toolbar_labels.check"), None);
    let (label, check, text) =
        (t.tr("prefs.ui.reduce_motion.label"), t.tr("prefs.ui.reduce_motion.check"), t.tr("prefs.ui.reduce_motion.help"));
    check_row(ui, label, &mut s.reduce_motion, check, Some(text));
    let (label, check, text) = (t.tr("prefs.ui.spellcheck.label"), t.tr("prefs.ui.spellcheck.check"), t.tr("prefs.ui.spellcheck.help"));
    check_row(ui, label, &mut s.spellcheck, check, Some(text));
    let (label, check, text) = (t.tr("prefs.ui.initials.label"), t.tr("prefs.ui.initials.check"), t.tr("prefs.ui.initials.help"));
    check_row(ui, label, &mut s.use_initials, check, Some(text));
    let (label, check, text) = (t.tr("prefs.ui.ghost.label"), t.tr("prefs.ui.ghost.check"), t.tr("prefs.ui.ghost.help"));
    check_row(ui, label, &mut s.ghost_refs, check, Some(text));
    check_row(ui, t.tr("prefs.ui.highlight_rows.label"), &mut s.highlight_ref_rows, t.tr("prefs.ui.highlight_rows.check"), None);
    let (label, check, text) =
        (t.tr("prefs.ui.worktree_wips.label"), t.tr("prefs.ui.worktree_wips.check"), t.tr("prefs.ui.worktree_wips.help"));
    check_row(ui, label, &mut s.worktree_wips, check, Some(text));
    let (label, check, text) = (t.tr("prefs.ui.group_tabs.label"), t.tr("prefs.ui.group_tabs.check"), t.tr("prefs.ui.group_tabs.help"));
    check_row(ui, label, &mut s.group_worktree_tabs, check, Some(text));
    row(ui, t.tr("prefs.ui.branch_visibility.label"), |ui| {
        let options = [(BV::All, t.tr("prefs.ui.branch_visibility.all")), (BV::Smart, t.tr("prefs.ui.branch_visibility.smart"))];
        combo(ui, "visibility", &mut s.branch_visibility, &options);
        help(ui, t.tr("prefs.ui.branch_visibility.help"));
    });
}
