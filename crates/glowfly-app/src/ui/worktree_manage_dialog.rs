//! The Remove worktree dialog (what goes, why force is needed, whether the branch and its copy on
//! the remote are deleted too, and that another tab showing it closes) and the Lock worktree
//! dialog (an optional reason), each with a side pane that explains the step. `update` checks
//! the request again and closes the dialog once the job starts.

use super::dialog_frame::{self, Cx, Dialog, Footer};
use super::dialog_guides;
use super::theme::color;
use super::widgets::form_message::{self, Message};
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::Msg;
use crate::state::blocked::BUSY;
use crate::state::gitflow_base;
use crate::state::worktree_manage::{LockForm, RemoveForm};
use crate::ui::theme::controls;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Id, RichText};
use glowfly_core::i18n::Lang;
use glowfly_core::worktree_ops::Refusal;

/// The Remove dialog; returns true when Cancel closes it. The danger button is disabled while a
/// job runs (`idle` false), and in force mode until the acknowledgement is ticked.
pub fn remove(ui: &mut egui::Ui, cx: Cx, form: &mut RemoveForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut d = Dialog::new(cx, format!("Remove worktree {}", form.name)).guide(dialog_guides::worktree_remove(cx.t)).begin(ui);
    d.body(ui, |ui| body(ui, cx.t, form));
    if let Some(label) = form.ack_label() {
        dialog_frame::ack(ui, cx.t, &mut form.acked, Some(&label));
    }
    let ok = idle && (!form.needs_ack() || form.acked);
    let why = if idle { cx.t.tr("common.ack.tip") } else { BUSY };
    let out = d.finish(ui, Footer::new().cancel().danger(buttons::danger(form.button()).enabled(ok).why(Some(why))));
    if out.primary {
        msgs.push(Msg::Worktree(WorktreeMsg::Remove));
    }
    out.dismiss
}

/// What goes, why force is needed, and the branch options (with the Git Flow warning when the
/// branch to delete is the production or development branch).
fn body(ui: &mut egui::Ui, t: Lang, form: &mut RemoveForm) {
    ui.label(format!(
        "Deletes the folder {} and everything in it, and drops git's record of the worktree. Commits and branches stay in the repository.",
        glowfly_core::native_path::shown(&form.path)
    ));
    if let Some(note) = form.close_note() {
        ui.add_space(6.0);
        form_message::form(ui, Message::info(note));
    }
    if let Some(note) = form.force_note() {
        let locked = matches!(form.refusal, Some(Refusal::Locked(_)));
        ui.add_space(6.0);
        form_message::form(ui, if locked { Message::warning(note) } else { Message::error(note) });
    }
    if let Some(b) = form.branch.clone() {
        ui.add_space(8.0);
        controls::checkbox(ui, &mut form.delete_branch, format!("Also delete branch {b}"));
        ui.label(
            RichText::new("Only if it is merged (git branch -d). If it isn't, the worktree is still removed and you are told.")
                .color(color::weak()),
        );
        if form.delete_branch
            && let Some(r) = form.remote.clone()
        {
            ui.add_space(4.0);
            let label = format!("Also delete {} on {}", r.tracking.merge, r.tracking.remote);
            let c = ui.add_enabled(r.blocked.is_none(), controls::Check::new(&mut form.delete_remote, label));
            if let Some(why) = &r.blocked {
                c.on_disabled_hover_text(why);
            }
        }
        if let Some(note) = form.remote_note() {
            ui.add_space(4.0);
            form_message::form(ui, Message::warning(note));
        }
        if form.delete_branch
            && let Some(role) = form.flow_base
        {
            ui.add_space(4.0);
            form_message::form(ui, Message::warning(gitflow_base::warning(t, role, &b)));
        }
    }
}

/// The Lock dialog; returns true when Cancel closes it. The reason field has the keyboard focus
/// unless `cx.keyboard` is false (a credential prompt is open above it); Enter locks.
pub fn lock(ui: &mut egui::Ui, cx: Cx, form: &mut LockForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, format!("Lock worktree {}", form.name)).guide(dialog_guides::worktree_lock(cx.t)).begin(ui);
    let r = FieldGroup::new("Reason")
        .optional(cx.t.tr("dialog.field.optional"))
        .show(ui, |ui| ui.add(TextField::new(&mut form.reason).id(Id::new("trellis-worktree-lock-reason")).hint("e.g. on the USB drive")));
    let enter = text_field::entered(ui, &r);
    if cx.keyboard {
        r.request_focus();
    }
    let lock = buttons::primary("Lock").enabled(idle).why((!idle).then_some(BUSY));
    let out = d.finish(ui, Footer::new().cancel().primary(lock).submit(enter));
    if out.primary {
        msgs.push(Msg::Worktree(WorktreeMsg::Lock));
    }
    out.dismiss
}
