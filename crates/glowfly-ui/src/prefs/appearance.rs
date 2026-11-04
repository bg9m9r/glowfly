//! Preferences > Appearance: the theme, Interface (with its Interface size row, which
//! Accessibility shows too: `zoom_row`) and the commit graph's look (appearance_graph.rs).
//! Notifications and spell checking are on General.

use super::{check_row, help, row, section};
use crate::theme::controls;
use eframe::egui::{self, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;
use glowfly_model::keymap::{BindingId, Keymap};
use glowfly_model::msg::Msg;
use glowfly_model::themes::ThemeState;

/// The Interface rows that depend on the window frame.
#[derive(Clone, Copy)]
pub struct FrameRows {
    /// The system frame's menu bar can be turned off (a drawn title bar has none).
    pub menu_bar: bool,
    /// Windows can use its own title bar instead of the drawn one (from the next start).
    pub system_title_bar: bool,
}

pub fn ui_page(ui: &mut Ui, s: &mut Settings, themes: &ThemeState, keys: &Keymap, frame: FrameRows, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    super::theme_row::theme_row(ui, s, themes, msgs);
    interface_section(ui, t, s, keys, frame);
    super::appearance_graph::graph_section(ui, t, s);
}

/// Interface size: the window zoom, with its keys in the help (only the first sentence when
/// either has no key). The same row on Appearance and Accessibility; `follow` adds
/// Accessibility's "Also follow the system's text size" check under the combo.
pub fn zoom_row(ui: &mut Ui, t: Lang, s: &mut Settings, keys: &Keymap, follow: bool) {
    let label = t.tr("prefs.a11y.zoom.label");
    let text = match (keys.hint(BindingId::ZoomIn), keys.hint(BindingId::ZoomOut)) {
        (Some(zoom_in), Some(zoom_out)) => t.trf("prefs.a11y.zoom.help", &[("in", &zoom_in), ("out", &zoom_out)]),
        _ => t.tr("prefs.a11y.zoom.help_no_keys").to_owned(),
    };
    row(ui, label, |ui| {
        let combo = egui::ComboBox::from_id_salt("zoom").selected_text(format!("{}%", s.zoom_level())).width(super::fit(ui, 230.0));
        let r = combo.show_ui(ui, |ui| {
            for p in glowfly_core::settings::ZOOM_STEPS {
                ui.selectable_value(&mut s.zoom_percent, p, format!("{p}%"));
            }
        });
        super::accessibility::name(&r.response, label, &text);
        if follow {
            super::accessibility::follow_check(ui, label, &mut s.a11y.follow_system_text_size, t.tr("prefs.a11y.zoom.follow"), &text);
        }
        help(ui, &text);
    });
}

/// Interface: zoom, the panels and toolbar, the Windows title bar, the menu bar, toolbar labels,
/// avatars, worktree tabs, new groups' colour and how changed-file lists show their files.
fn interface_section(ui: &mut Ui, t: Lang, s: &mut Settings, keys: &Keymap, frame: FrameRows) {
    section(ui, t.tr("prefs.ui.interface.section"));
    zoom_row(ui, t, s, keys, false);
    check_row(ui, t.tr("prefs.ui.left_panel.label"), &mut s.show_left_panel, t.tr("prefs.ui.left_panel.check"), None);
    check_row(ui, t.tr("prefs.ui.details_panel.label"), &mut s.show_details_panel, t.tr("prefs.ui.details_panel.check"), None);
    let (label, check, text) = (t.tr("prefs.ui.toolbar.label"), t.tr("prefs.ui.toolbar.check"), t.tr("prefs.ui.toolbar.help"));
    check_row(ui, label, &mut s.show_toolbar, check, Some(text));
    if frame.system_title_bar {
        let (label, check, text) = (t.tr("prefs.ui.title_bar.label"), t.tr("prefs.ui.title_bar.check"), t.tr("prefs.ui.title_bar.help"));
        check_row(ui, label, &mut s.system_title_bar, check, Some(text));
    }
    if frame.menu_bar {
        let (label, check, text) = (t.tr("prefs.ui.menu_bar.label"), t.tr("prefs.ui.menu_bar.check"), t.tr("prefs.ui.menu_bar.help"));
        check_row(ui, label, &mut s.show_menu_bar, check, Some(text));
    }
    check_row(ui, t.tr("prefs.ui.toolbar_labels.label"), &mut s.toolbar_labels, t.tr("prefs.ui.toolbar_labels.check"), None);
    let (label, check, text) = (t.tr("prefs.ui.avatars.label"), t.tr("prefs.ui.avatars.check"), t.tr("prefs.ui.avatars.help"));
    check_row(ui, label, &mut s.avatar_lookups, check, Some(text));
    let (label, check, text) = (t.tr("prefs.ui.group_tabs.label"), t.tr("prefs.ui.group_tabs.check"), t.tr("prefs.ui.group_tabs.help"));
    check_row(ui, label, &mut s.group_worktree_tabs, check, Some(text));
    super::group_colour_row::group_colour_row(ui, t, s);
    row(ui, t.tr("prefs.ui.file_lists.label"), |ui| {
        use glowfly_core::settings::FileListMode as FL;
        ui.horizontal(|ui| {
            controls::radio_value(ui, &mut s.file_list_mode, FL::List, t.tr("prefs.ui.file_lists.list"));
            controls::radio_value(ui, &mut s.file_list_mode, FL::Tree, t.tr("prefs.ui.file_lists.tree"));
        });
        help(ui, t.tr("prefs.ui.file_lists.help"));
    });
}
