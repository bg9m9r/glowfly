//! The Reword dialog (`Modal::RewordCommit`): a new message for a commit chosen in the graph,
//! or, as the Squash dialog, the message of the commit a squash makes (pre-filled with the
//! joined messages). It says which commits the rebase rewrites and, on a dirty tree, that the
//! changes are stashed first; Ctrl/Cmd+Enter submits (the dialog frame takes it before the
//! field sees it). `update` checks the form again before running anything.

use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::needs_stash;
use crate::state::rebase_quick::{BLANK, RewordForm, STASH_NOTE};
use crate::ui::dialog_frame::{Cx, Dialog, Footer};
use crate::ui::theme::color;
use crate::ui::widgets::form_message::{self, Message};
use crate::ui::widgets::text_field::TextField;
use crate::ui::widgets::{FieldGroup, buttons};
use eframe::egui::{self, RichText};
use glowfly_core::repo::Snapshot;

/// Draws the dialog; returns true when it should close (Cancel). The message field takes the
/// keyboard focus once, unless `cx.keyboard` is false (a credential prompt is open above it).
/// The primary button waits while a job runs (`idle` false); on submit the dialog stays open
/// until `update` starts the rebase.
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut RewordForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, form.title()).subtitle(form.subtitle()).begin(ui);
    let hint = form.hint();
    let problem = form.draft.trim().is_empty().then(|| Message::error(BLANK));
    let r = FieldGroup::new("Message")
        .message(&form.touched, "message", problem)
        .show(ui, |ui| ui.add(TextField::multiline(&mut form.draft, 6).hint(hint)));
    form_message::track(&mut form.touched, "message", &r);
    if cx.keyboard && std::mem::take(&mut form.focus) {
        r.request_focus();
    }
    let stash = snap.is_some_and(needs_stash);
    if stash {
        ui.add_space(4.0);
        ui.label(RichText::new(STASH_NOTE).color(color::weak()));
    }
    let blocked = form.blocker(idle);
    // A blank message's reason shows under the field once it is touched; the footer says it until then.
    let why = blocked.filter(|b| *b != BLANK || !form.touched.shows("message"));
    let button = buttons::primary(form.button(stash)).enabled(blocked.is_none()).why(why);
    let out = d.finish(ui, Footer::new().cancel().primary(button));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Rebase(RebaseMsg::SubmitReword));
    }
    out.dismiss
}
