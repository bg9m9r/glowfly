//! The dialogs a trellis:// link may need (`Modal::DeepLink`): a link Trellis can't open, a
//! repository not found here (locate it on disk or clone it), and several copies to choose
//! from. Esc closes them through the usual `CloseModal`, which gives the link up.

use super::common::capitalize;
use super::theme::color;
use crate::deep_link::DeepLinkMsg;
use crate::msg::Msg;
use crate::state::deep_link::{LinkDialog, NO_CLONE_URL};
use crate::ui::theme::controls;
use eframe::egui::{self, RichText};

/// Draws dialog `d`; true when it is to close (Close or Cancel). The other buttons leave it
/// open: their messages replace or close it.
pub fn show(ui: &mut egui::Ui, d: &LinkDialog, msgs: &mut Vec<Msg>) -> bool {
    let link = |m: DeepLinkMsg| Msg::DeepLink(m);
    let mut close = false;
    match d {
        LinkDialog::Invalid { text, reason } => {
            ui.heading("Can't open this link");
            ui.add_space(6.0);
            ui.add(egui::Label::new(format!("Trellis can't open this link: {reason}.")).wrap());
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(text.as_str()).monospace().color(color::weak())).wrap());
            ui.add_space(12.0);
            close = ui.button("Close").clicked();
        }
        LinkDialog::NotFound { what, repo, can_clone, problem } => {
            ui.heading("Repository not found");
            ui.add_space(6.0);
            ui.label("No open, recent or favorite repository matches this link.");
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(format!("{} in {repo}", capitalize(what))).color(color::weak())).wrap());
            if let Some(p) = problem {
                ui.add_space(6.0);
                ui.add(egui::Label::new(RichText::new(p.as_str()).color(color::red())).wrap());
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Locate on disk…").clicked() {
                    msgs.push(link(DeepLinkMsg::Locate));
                }
                let clone = ui.add_enabled(*can_clone, egui::Button::new("Clone…")).on_disabled_hover_text(NO_CLONE_URL);
                if clone.clicked() {
                    msgs.push(link(DeepLinkMsg::Clone));
                }
                close = ui.button("Cancel").clicked();
            });
        }
        LinkDialog::Choose { what, paths, picked } => {
            ui.heading("Choose a repository");
            ui.add_space(6.0);
            ui.add(egui::Label::new(format!("These folders are all copies of the repository with {what}. Which one should open?")).wrap());
            ui.add_space(6.0);
            for (i, p) in paths.iter().enumerate() {
                if controls::radio(ui, *picked == i, trellis_core::native_path::shown(p)).clicked() {
                    msgs.push(link(DeepLinkMsg::Choose(i)));
                }
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Open").clicked() {
                    msgs.push(link(DeepLinkMsg::OpenChosen));
                }
                close = ui.button("Cancel").clicked();
            });
        }
    }
    close
}
