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
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText};
use trellis_core::repo::Snapshot;

/// Draws the dialog; returns true when it should close (Cancel). The message field takes the
/// keyboard focus once, unless `cx.keyboard` is false (a credential prompt is open above it).
/// The primary button waits while a job runs (`idle` false); on submit the dialog stays open
/// until `update` starts the rebase.
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut RewordForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, form.title()).subtitle(form.subtitle()).begin(ui);
    let hint = form.hint();
    let r = controls::field(ui, egui::TextEdit::multiline(&mut form.draft).hint_text(hint).desired_rows(6).desired_width(f32::INFINITY));
    form_message::track(&mut form.touched, "message", &r);
    if cx.keyboard && std::mem::take(&mut form.focus) {
        r.request_focus();
    }
    let problem = form.draft.trim().is_empty().then(|| Message::error(BLANK));
    form_message::field(ui, &form.touched, "message", problem);
    let stash = snap.is_some_and(needs_stash);
    if stash {
        ui.add_space(4.0);
        ui.label(RichText::new(STASH_NOTE).color(color::weak()));
    }
    let blocked = form.blocker(idle);
    let button = buttons::primary(form.button(stash)).enabled(blocked.is_none()).why(blocked);
    let out = d.finish(ui, Footer::new().cancel().primary(button));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Rebase(RebaseMsg::SubmitReword));
    }
    out.dismiss
}
