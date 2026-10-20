//! The No Trash on this drive dialog (`Modal::TrashOffer`): the trash on a file's drive can't be
//! used, so the file can go to the Trash in the home folder (copied there) or be deleted
//! permanently instead.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::widgets::buttons;
use crate::msg::{Msg, WorkFileMsg};
use crate::state::files::TrashOffer;
use eframe::egui;

/// Draws the dialog; returns true when it should close (a choice was made, or Cancel).
pub fn show(ui: &mut egui::Ui, cx: Cx, offer: &TrashOffer, msgs: &mut Vec<Msg>) -> bool {
    let mut d = Dialog::new(cx, "No Trash on this drive").size(Size::Small).begin(ui);
    d.body(ui, |ui| ui.add(egui::Label::new(offer.body.as_str()).wrap()));
    let footer = Footer::new().cancel().alt(buttons::danger("Delete permanently")).primary(buttons::primary("Move to home Trash"));
    let out = d.finish(ui, footer);
    if out.primary {
        msgs.push(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(offer.op.clone().home_trash())));
    }
    if out.alt == Some(0) {
        msgs.push(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(offer.op.clone().permanent())));
    }
    out.primary || out.alt.is_some() || out.dismiss
}
