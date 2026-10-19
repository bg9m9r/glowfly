//! The Rename stash dialog (`Modal::RenameStash`): one field for the stash list text after
//! "On <branch>: ". `update` checks the text again and re-finds the stash before running anything.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::widgets::buttons;
use super::widgets::form_message::{self, Message};
use crate::msg::{Msg, StashMsg};
use crate::state::stashes::RenameStashForm;
use crate::ui::theme::controls;
use eframe::egui::{self, Key};

/// Draws the dialog; returns true when it should close. Its field keeps the keyboard focus unless
/// `cx.keyboard` is false (a credential prompt is open above it). Rename waits while a job runs
/// (`idle` is false); on submit the dialog stays open until `update` starts the rename.
pub fn rename(ui: &mut egui::Ui, cx: Cx, form: &mut RenameStashForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let at = match form.branch() {
        Some(b) => format!("stash@{{{}}} · made on {b}", form.index),
        None => format!("stash@{{{}}}", form.index),
    };
    let d = Dialog::new(cx, "Rename stash").subtitle(at).begin(ui);
    let r = controls::field(ui, egui::TextEdit::singleline(&mut form.message).hint_text("Stash message").desired_width(f32::INFINITY));
    form_message::track(&mut form.touched, "message", &r);
    // Read before focus is taken back, or the Enter that just left the field is missed.
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if cx.keyboard {
        r.request_focus();
    }
    let problem = form.new_message().is_none().then(|| Message::error("Enter a message."));
    form_message::field(ui, &form.touched, "message", problem);
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
