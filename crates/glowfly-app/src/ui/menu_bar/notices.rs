//! Help > About > Third-party notices (`Modal::Notices`): the licenses of the bundled fonts and
//! syntax definitions, one folded header each. Back returns to About; the dialog can be resized.

use crate::msg::{AppMsg, Msg};
use crate::state::menu_bar::reflow;
use crate::ui::dialog_frame::{Cx, Dialog, Footer, Size};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, CollapsingHeader, Label, RichText};
use glowfly_core::notices;

/// Draws the dialog in `cx.t`; true when it should close.
pub(crate) fn notices(ui: &mut egui::Ui, cx: Cx, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let mut d = Dialog::new(cx, t.tr("about.notices.title")).size(Size::Medium).resizable("notices").begin(ui);
    d.body(ui, |ui| {
        ui.add(Label::new(RichText::new(t.tr("about.notices.intro")).color(color::weak())).wrap());
        for n in notices::licenses() {
            CollapsingHeader::new(&n.name).id_salt(&n.source).default_open(false).show(ui, |ui| {
                ui.add(Label::new(RichText::new(reflow(&n.text)).size(type_scale::SMALL)).wrap());
            });
        }
    });
    let footer = Footer::new()
        .extra(buttons::secondary(t.tr("about.notices.back.button")))
        .extra(buttons::secondary(t.tr("about.copy_notices.button")))
        .close();
    let out = d.finish(ui, footer);
    match out.extra {
        Some(0) => msgs.push(Msg::App(AppMsg::ShowAbout)),
        Some(_) => msgs.push(Msg::Copy(notices::TEXT.to_owned())),
        None => {}
    }
    out.dismiss
}
