//! Preferences > General: the Trellis Links row (which program opens trellis:// links, and
//! registering this copy of Trellis for them).

use super::{help, row};
use crate::deep_link::DeepLinkMsg;
use crate::msg::Msg;
use crate::state::deep_link::{SchemeView, can_unregister, status_text};
use crate::ui::theme::color;
use eframe::egui::{self, RichText, Ui};
use trellis_core::url_scheme::{MACOS_NOTE, SchemeStatus};

const HELP: &str =
    "Lets links from a browser or chat open a repository, branch, commit or tag in Trellis. Only your user account is affected.";

pub fn links_row(ui: &mut Ui, view: &SchemeView, msgs: &mut Vec<Msg>) {
    row(ui, "Trellis Links", |ui| {
        if cfg!(target_os = "macos") || matches!(view, SchemeView::Known(SchemeStatus::Unsupported(_))) {
            ui.add(egui::Label::new(RichText::new(MACOS_NOTE).color(color::text())).wrap());
            help(ui, HELP);
            return;
        }
        let (text, error) = status_text(view);
        ui.add(egui::Label::new(RichText::new(text).color(if error { color::red() } else { color::text() })).wrap());
        let checking = matches!(view, SchemeView::Unknown | SchemeView::Checking);
        let other = matches!(view, SchemeView::Known(SchemeStatus::OtherProgram(_)));
        ui.horizontal(|ui| {
            let register = if other { "Register this copy" } else { "Register" };
            if ui.add_enabled(!checking, egui::Button::new(register)).clicked() {
                msgs.push(Msg::DeepLink(DeepLinkMsg::Register));
            }
            if ui.add_enabled(!checking && can_unregister(view), egui::Button::new("Unregister")).clicked() {
                msgs.push(Msg::DeepLink(DeepLinkMsg::Unregister));
            }
            if error && ui.button("Check again").clicked() {
                msgs.push(Msg::DeepLink(DeepLinkMsg::CheckScheme));
            }
        });
        help(ui, HELP);
    });
}
