//! The Remove worktree dialog (what goes, why force is needed, and whether the branch is deleted
//! too) and the Lock worktree dialog (an optional reason). `update` checks the request again and
//! closes the dialog once the job starts.

use super::theme::color;
use crate::msg::Msg;
use crate::state::worktree_manage::{LockForm, RemoveForm};
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::worktree_ops::Refusal;

/// The Remove dialog; returns true when Cancel closes it. The button is disabled while a job
/// runs (`idle` false).
pub fn remove(ui: &mut egui::Ui, form: &mut RemoveForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(format!("Remove worktree {}", form.name));
    ui.add_space(6.0);
    ui.label(format!(
        "Deletes the folder {} and everything in it, and drops git's record of the worktree. Commits and branches stay in the repository.",
        form.path.display()
    ));
    if let Some(note) = form.force_note() {
        let col = if matches!(form.refusal, Some(Refusal::Locked(_))) { color::amber() } else { color::red() };
        ui.add_space(6.0);
        ui.label(RichText::new(note).color(col));
    }
    if let Some(b) = form.branch.clone() {
        ui.add_space(8.0);
        ui.checkbox(&mut form.delete_branch, format!("Also delete branch {b}"));
        ui.label(
            RichText::new("Only if it is merged (git branch -d). If it isn't, the worktree is still removed and you are told.")
                .color(color::weak()),
        );
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let mut text = RichText::new(form.button());
        if form.refusal.is_some() {
            text = text.color(color::red());
        }
        if ui.add_enabled(idle, egui::Button::new(text)).clicked() {
            msgs.push(Msg::Worktree(WorktreeMsg::Remove));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

/// The Lock dialog; returns true when Cancel closes it. The reason field has the keyboard focus
/// unless `focus` is false (a credential prompt is open above it); Enter locks.
pub fn lock(ui: &mut egui::Ui, form: &mut LockForm, focus: bool, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(format!("Lock worktree {}", form.name));
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "git won't prune a locked worktree when its folder is missing (for example on a removable drive), and removing it needs Force remove.",
        )
        .color(color::weak()),
    );
    ui.add_space(8.0);
    ui.label(RichText::new("Reason (optional)").color(color::weak()));
    let r = ui.add(
        egui::TextEdit::singleline(&mut form.reason)
            .id(Id::new("trellis-worktree-lock-reason"))
            .hint_text("e.g. on the USB drive")
            .desired_width(f32::INFINITY),
    );
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus {
        r.request_focus();
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(idle, egui::Button::new("Lock")).clicked() || (enter && idle) {
            msgs.push(Msg::Worktree(WorktreeMsg::Lock));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
