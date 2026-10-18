//! The No Trash on this drive dialog (`Modal::TrashOffer`): the trash on a file's drive can't be
//! used, so the file can go to the Trash in the home folder (copied there) or be deleted
//! permanently instead.

use crate::msg::{Msg, WorkFileMsg};
use crate::state::files::TrashOffer;
use eframe::egui;

/// Draws the dialog; returns true when it should close (a choice was made, or Cancel).
pub fn show(ui: &mut egui::Ui, offer: &TrashOffer, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("No Trash on this drive");
    ui.add_space(6.0);
    ui.label(offer.body.as_str());
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("Move to home Trash").clicked() {
            msgs.push(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(offer.op.clone().home_trash())));
            close = true;
        }
        if ui.button("Delete permanently").clicked() {
            msgs.push(Msg::WorkFile(WorkFileMsg::DeleteConfirmed(offer.op.clone().permanent())));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
