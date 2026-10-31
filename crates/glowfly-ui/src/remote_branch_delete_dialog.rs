//! The Delete remote branch dialog (`Modal::DeleteRemoteBranch`), in the danger confirmation's
//! form: the question, a warning naming the local branches that track the branch with a
//! checkbox (on at first) to unset their upstreams once it is deleted, and the Git Flow warning
//! for a base branch. The danger button sits at the right edge; Cancel is focused and is what
//! Enter presses.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::controls;
use super::widgets::buttons;
use super::widgets::form_message::{self, Message};
use crate::msg::Msg;
use crate::state::branch_sync::DeleteRemoteForm;
use eframe::egui::{self, Ui};

/// Draws the dialog; returns true when it should close (Cancel, or the delete was sent).
pub(super) fn show(ui: &mut Ui, cx: Cx, form: &mut DeleteRemoteForm, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let mut d = Dialog::new(cx, form.title(t)).size(Size::Small).begin(ui);
    d.body(ui, |ui| {
        ui.add(egui::Label::new(form.body(t)).wrap());
        if let Some(note) = form.tracking_note(t) {
            ui.add_space(8.0);
            form_message::form(ui, Message::warning(note));
            ui.add_space(4.0);
            let label = form.unset_label(t);
            controls::checkbox(ui, &mut form.unset, label);
        }
        if let Some(warn) = form.role_warning(t) {
            ui.add_space(8.0);
            form_message::form(ui, Message::warning(warn));
        }
    });
    let out = d.finish(ui, Footer::new().danger(buttons::danger(form.button(t))).cancel());
    if out.primary {
        msgs.push(form.delete_msg());
    }
    out.primary || out.dismiss
}
