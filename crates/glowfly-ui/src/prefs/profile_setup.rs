//! Preferences > Profile: the selected profile's Accounts and Preferences rows, which say what
//! switches with it. Read-only: accounts are connected in Integrations, preferences on their
//! own pages.

use super::{help, row};
use crate::state::profiles::{
    ACCOUNTS_CURRENT_HELP, ACCOUNTS_INHERIT, ACCOUNTS_LABEL, ACCOUNTS_OTHER_HELP, NO_ACCOUNTS, PREFS_LABEL, SetupView, account_line,
    accounts_of, prefs_status,
};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use eframe::egui::{RichText, Ui};
use glowfly_core::settings::{Profile, Settings};

pub(super) fn setup_rows(ui: &mut Ui, settings: &Settings, p: &Profile, current: bool) {
    row(ui, ACCOUNTS_LABEL, |ui| {
        // Line up the first line with the label.
        ui.add_space(3.0);
        let line = |ui: &mut Ui, text: &str, faint: bool| {
            let c = if faint { color::faint() } else { color::text() };
            ui.label(RichText::new(text).size(type_scale::BODY).color(c));
        };
        match accounts_of(settings, p.id) {
            SetupView::Live(list) | SetupView::Saved(list) if list.is_empty() => line(ui, NO_ACCOUNTS, true),
            SetupView::Live(list) | SetupView::Saved(list) => {
                for c in list {
                    line(ui, &account_line(c), false);
                }
            }
            SetupView::Inherit => line(ui, ACCOUNTS_INHERIT, true),
        }
        ui.add_space(2.0);
        help(ui, if current { ACCOUNTS_CURRENT_HELP } else { ACCOUNTS_OTHER_HELP });
    });
    row(ui, PREFS_LABEL, |ui| {
        ui.add_space(3.0);
        help(ui, prefs_status(settings, p.id));
    });
}
