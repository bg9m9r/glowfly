//! The Rename stash dialog (`Modal::RenameStash`): one field for the stash list text after
//! "On <branch>: ". `update` checks the text again and re-finds the stash before running anything.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::widgets::form_message::{self, Message};
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::{Msg, StashMsg};
use crate::state::stashes::RenameStashForm;
use eframe::egui;

/// Draws the dialog; returns true when it should close. Its field keeps the keyboard focus unless
/// `cx.keyboard` is false (a credential prompt is open above it). Rename waits while a job runs
/// (`idle` is false); on submit the dialog stays open until `update` starts the rename.
pub fn rename(ui: &mut egui::Ui, cx: Cx, form: &mut RenameStashForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let at = match form.branch() {
        Some(b) => format!("stash@{{{}}} · made on {b}", form.index),
        None => format!("stash@{{{}}}", form.index),
    };
    let d = Dialog::new(cx, "Rename stash").subtitle(at).size(Size::Small).begin(ui);
    let problem = form.new_message().is_none().then(|| Message::error("Enter a message."));
    let r = FieldGroup::new("Message").message(&form.touched, "message", problem).show(ui, |ui| ui.add(TextField::new(&mut form.message)));
    form_message::track(&mut form.touched, "message", &r);
    // Read before focus is taken back, or the Enter that just left the field is missed.
    let submit = text_field::entered(ui, &r);
    if cx.keyboard {
        r.request_focus();
    }
    let valid = form.changed() && idle;
    let why = (!idle).then_some("Another operation is still running.");
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Rename").enabled(valid).why(why)).submit(submit));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Stash(StashMsg::Rename { oid: form.oid, message: form.message.clone() }));
    }
    out.dismiss
}
