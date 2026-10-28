//! The branch Delete dialog (`Modal::DeleteBranch`), in the danger confirmation's form: the
//! question, a spinner line while the commits only the branch has are counted, then a warning
//! with their count (or why it is unknown) and the Git Flow warning for a base branch. The
//! danger button sits at the right edge, Delete when nothing is lost and Delete anyway when
//! something may be, disabled while counting; Cancel is focused and is what Enter presses.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::pane_status::loading_line;
use super::widgets::buttons;
use super::widgets::form_message::{self, Message};
use crate::msg::Msg;
use crate::state::branch_delete::{DeleteForm, Lost};
use crate::state::gitflow_base;
use eframe::egui::{self, Ui};

/// Draws the dialog; returns true when it should close (Cancel).
pub(super) fn show(ui: &mut Ui, cx: Cx, form: &DeleteForm, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let mut d = Dialog::new(cx, form.title(t)).size(Size::Small).begin(ui);
    d.body(ui, |ui| {
        ui.add(egui::Label::new(t.trf("branch.delete.body", &[("branch", &form.name)])).wrap());
        if form.lost == Lost::Counting {
            ui.add_space(6.0);
            loading_line(ui, t.tr("branch.delete.checking"));
        }
        if let Some(note) = form.lost_note(t) {
            ui.add_space(8.0);
            form_message::form(ui, Message::warning(note));
        }
        if let Some(role) = form.role {
            ui.add_space(8.0);
            form_message::form(ui, Message::warning(gitflow_base::warning(t, role, &form.name)));
        }
    });
    let send = form.delete_msg();
    let button = buttons::danger(form.button(t)).enabled(send.is_some()).why(Some(t.tr("branch.delete.checking")));
    let out = d.finish(ui, Footer::new().danger(button).cancel());
    if out.primary
        && let Some(m) = send
    {
        msgs.push(m);
    }
    out.dismiss
}
