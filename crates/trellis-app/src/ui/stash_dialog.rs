//! The Rename stash dialog (`Modal::RenameStash`): one field for the stash list text after
//! "On <branch>: ". `update` checks the text again and re-finds the stash before running anything.

use super::theme::color;
use crate::msg::{Msg, StashMsg};
use crate::state::stashes::RenameStashForm;
use crate::ui::theme::controls;
use eframe::egui::{self, Key, RichText};

/// Draws the dialog; returns true when it should close. Its field keeps the keyboard focus unless
/// `focus` is false (a credential prompt is open above it). Rename waits while a job runs (`idle`
/// is false); on submit the dialog stays open until `update` starts the rename.
pub fn rename(ui: &mut egui::Ui, form: &mut RenameStashForm, focus: bool, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Rename stash");
    let at = match form.branch() {
        Some(b) => format!("stash@{{{}}} · made on {b}", form.index),
        None => format!("stash@{{{}}}", form.index),
    };
    ui.label(RichText::new(at).color(color::weak()));
    ui.add_space(6.0);
    let r = controls::field(ui, egui::TextEdit::singleline(&mut form.message).hint_text("Stash message").desired_width(f32::INFINITY));
    // Read before focus is taken back, or the Enter that just left the field is missed.
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus {
        r.request_focus();
    }
    if form.new_message().is_none() {
        ui.add_space(4.0);
        ui.label(RichText::new("Enter a message.").color(color::red()));
    }
    ui.add_space(10.0);
    let valid = form.changed() && idle;
    ui.horizontal(|ui| {
        let mut button = ui.add_enabled(valid, egui::Button::new("Rename"));
        if !idle {
            button = button.on_disabled_hover_text("Another operation is still running.");
        }
        if button.clicked() || (submit && valid) {
            msgs.push(Msg::Stash(StashMsg::Rename { oid: form.oid, message: form.message.clone() }));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
