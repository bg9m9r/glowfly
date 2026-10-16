//! The dialog before undoing a checkout that created a branch (`Modal::UndoCreated`): it switches
//! back, and also deletes the branch while the box is ticked. `update` checks again that the
//! branch still points where it was created before deleting it.

use super::theme::color;
use crate::effect::UndoOpts;
use crate::msg::{JobMsg, Msg};
use crate::state::undo::UndoCreatedForm;
use eframe::egui::{self, RichText};

/// Draws the dialog; returns true when it should close.
pub fn created_branch(ui: &mut egui::Ui, form: &mut UndoCreatedForm, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(format!("Undo checkout of new branch {}", form.branch));
    ui.add_space(6.0);
    ui.label(format!("Switches back to {}.", form.back_to));
    ui.add_space(8.0);
    ui.checkbox(&mut form.delete, format!("Also delete branch {}", form.branch));
    // Lined up with the checkbox's text.
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
        ui.label(RichText::new("It still points where this checkout created it, so no commits are lost.").color(color::weak()));
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("Undo").clicked() {
            let opts = UndoOpts { delete_created: form.delete, stash_first: false };
            msgs.push(Msg::Job(JobMsg::UndoWith { redo: false, action: Box::new(form.action.clone()), opts }));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
