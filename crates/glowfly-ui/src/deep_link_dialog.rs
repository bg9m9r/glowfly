//! The dialogs a glowfly:// link may need (`Modal::DeepLink`): a link Glowfly can't open, a
//! repository not found here (locate it on disk or clone it), and several copies to choose
//! from. Esc closes them through the usual `CloseModal`, which gives the link up.

use super::common::capitalize;
use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::buttons;
use crate::deep_link::DeepLinkMsg;
use crate::msg::{Msg, UpdateMsg};
use crate::state::deep_link::{LinkDialog, NO_CLONE_URL, pick_reason_text};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText};

/// Draws dialog `d`; true when it is to close (Close or Cancel). The other buttons leave it
/// open: their messages replace or close it.
pub fn show(ui: &mut egui::Ui, cx: Cx, d: &LinkDialog, msgs: &mut Vec<Msg>) -> bool {
    let link = |m: DeepLinkMsg| Msg::DeepLink(m);
    match d {
        LinkDialog::Invalid { text, unknown_kind: Some(kind), .. } => {
            let t = cx.t;
            let d = Dialog::new(cx, t.tr("deep_link.newer.title")).size(Size::Small).begin(ui);
            ui.add(egui::Label::new(t.trf("deep_link.newer.body", &[("kind", kind)])).wrap());
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(text.as_str()).monospace().color(color::weak())).wrap());
            let out = d.finish(ui, Footer::new().close().primary(buttons::primary(t.tr("deep_link.newer.check.button"))));
            if out.primary {
                msgs.push(Msg::Updates(UpdateMsg::CheckNow));
            }
            out.dismiss || out.primary
        }
        LinkDialog::Invalid { text, reason, unknown_kind: None } => {
            let d = Dialog::new(cx, "Can't open this link").size(Size::Small).begin(ui);
            ui.add(egui::Label::new(format!("Glowfly can't open this link: {reason}.")).wrap());
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(text.as_str()).monospace().color(color::weak())).wrap());
            d.finish(ui, Footer::new().close()).dismiss
        }
        LinkDialog::NotFound { what, repo, can_clone, problem } => {
            let d = Dialog::new(cx, "Repository not found").size(Size::Small).begin(ui);
            ui.label("No open, recent or favorite repository matches this link.");
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(format!("{} in {repo}", capitalize(what))).color(color::weak())).wrap());
            if let Some(p) = problem {
                ui.add_space(6.0);
                form_message::form(ui, Message::error(p.as_str()));
            }
            let footer = Footer::new()
                .cancel()
                .alt(buttons::secondary("Locate on disk…"))
                .primary(buttons::primary("Clone…").enabled(*can_clone).why(Some(NO_CLONE_URL)));
            let out = d.finish(ui, footer);
            if out.alt == Some(0) {
                msgs.push(link(DeepLinkMsg::Locate));
            }
            if out.primary {
                msgs.push(link(DeepLinkMsg::Clone));
            }
            out.dismiss
        }
        LinkDialog::Choose { what, paths, picked, reason } => {
            let mut d = Dialog::new(cx, "Choose a repository").begin(ui);
            ui.add(egui::Label::new(format!("These folders are all copies of the repository with {what}. Which one should open?")).wrap());
            ui.add_space(6.0);
            d.body(ui, |ui| {
                for (i, p) in paths.iter().enumerate() {
                    let why = reason.filter(|_| i == 0);
                    let radio = |ui: &mut egui::Ui| controls::radio(ui, *picked == i, glowfly_core::native_path::shown(p));
                    let r = match why {
                        Some(r) => {
                            ui.horizontal(|ui| {
                                let radio = radio(ui);
                                ui.label(RichText::new(pick_reason_text(cx.t, r)).size(type_scale::SMALL).color(color::weak()));
                                radio
                            })
                            .inner
                        }
                        None => radio(ui),
                    };
                    if r.clicked() {
                        msgs.push(link(DeepLinkMsg::Choose(i)));
                    }
                }
            });
            let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Open")));
            if out.primary {
                msgs.push(link(DeepLinkMsg::OpenChosen));
            }
            out.dismiss
        }
    }
}
