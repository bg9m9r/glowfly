//! The Remove worktree dialog (what goes, why force is needed, whether the branch and its copy on
//! the remote are deleted too, and that another tab showing it closes) and the Lock worktree
//! dialog (an optional reason). `update` checks the request again and
//! closes the dialog once the job starts.

use super::dialog_frame::{self, Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::Msg;
use crate::state::worktree_manage::{LockForm, RemoveForm};
use crate::ui::theme::controls;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::worktree_ops::Refusal;

/// The Remove dialog; returns true when Cancel closes it. The danger button is disabled while a
/// job runs (`idle` false), and in force mode until the acknowledgement is ticked.
pub fn remove(ui: &mut egui::Ui, cx: Cx, form: &mut RemoveForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut d = Dialog::new(cx, format!("Remove worktree {}", form.name)).begin(ui);
    d.body(ui, |ui| body(ui, form));
    if let Some(label) = form.ack_label() {
        dialog_frame::ack(ui, cx.t, &mut form.acked, Some(&label));
    }
    let ok = idle && (!form.needs_ack() || form.acked);
    let why = if idle { cx.t.tr("common.ack.tip") } else { "Another operation is still running." };
    let out = d.finish(ui, Footer::new().cancel().danger(buttons::danger(form.button()).enabled(ok).why(Some(why))));
    if out.primary {
        msgs.push(Msg::Worktree(WorktreeMsg::Remove));
    }
    out.dismiss
}

/// What goes, why force is needed, and the branch options.
fn body(ui: &mut egui::Ui, form: &mut RemoveForm) {
    ui.label(format!(
        "Deletes the folder {} and everything in it, and drops git's record of the worktree. Commits and branches stay in the repository.",
        trellis_core::native_path::shown(&form.path)
    ));
    if let Some(note) = form.close_note() {
        ui.add_space(6.0);
        ui.label(RichText::new(note).color(color::amber()));
    }
    if let Some(note) = form.force_note() {
        let col = if matches!(form.refusal, Some(Refusal::Locked(_))) { color::amber() } else { color::red() };
        ui.add_space(6.0);
        ui.label(RichText::new(note).color(col));
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
            ui.label(RichText::new(note).color(color::red()));
        }
    }
}

/// The Lock dialog; returns true when Cancel closes it. The reason field has the keyboard focus
/// unless `cx.keyboard` is false (a credential prompt is open above it); Enter locks.
pub fn lock(ui: &mut egui::Ui, cx: Cx, form: &mut LockForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, format!("Lock worktree {}", form.name)).begin(ui);
    ui.label(
        RichText::new(
            "git won't prune a locked worktree when its folder is missing (for example on a removable drive), and removing it needs Force remove.",
        )
        .color(color::weak()),
    );
    ui.add_space(8.0);
    ui.label(RichText::new("Reason (optional)").color(color::weak()));
    let r = controls::field(
        ui,
        egui::TextEdit::singleline(&mut form.reason)
            .id(Id::new("trellis-worktree-lock-reason"))
            .hint_text("e.g. on the USB drive")
            .desired_width(f32::INFINITY),
    );
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if cx.keyboard {
        r.request_focus();
    }
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Lock").enabled(idle)).submit(enter));
    if out.primary {
        msgs.push(Msg::Worktree(WorktreeMsg::Lock));
    }
    out.dismiss
}
