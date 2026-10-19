//! The dialog before undoing a checkout that created a branch (`Modal::UndoCreated`): it switches
//! back, and also deletes the branch while the box is ticked. `update` checks again that the
//! branch still points where it was created before deleting it.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::buttons;
use crate::effect::UndoOpts;
use crate::msg::{JobMsg, Msg};
use crate::state::undo::UndoCreatedForm;
use crate::ui::theme::controls;
use eframe::egui::{self, RichText};

/// Draws the dialog; returns true when it should close.
pub fn created_branch(ui: &mut egui::Ui, cx: Cx, form: &mut UndoCreatedForm, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, format!("Undo checkout of new branch {}", form.branch)).begin(ui);
    ui.label(format!("Switches back to {}.", form.back_to));
    ui.add_space(8.0);
    controls::checkbox(ui, &mut form.delete, format!("Also delete branch {}", form.branch));
    // Lined up with the checkbox's text.
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
        ui.label(RichText::new("It still points where this checkout created it, so no commits are lost.").color(color::weak()));
    });
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Undo")));
    if out.primary {
        let opts = UndoOpts { delete_created: form.delete, stash_first: false };
        msgs.push(Msg::Job(JobMsg::UndoWith { redo: false, action: Box::new(form.action.clone()), opts }));
    }
    out.primary || out.dismiss
}
