//! The Create worktree dialog: a folder (typed, or a parent picked with Browse…), an optional new
//! branch (required for a remote branch, which it then tracks) and whether to open the worktree
//! in a tab, with a side pane that explains the step. Problems are checked as the user types
//! with the pure rules in `state::worktrees`; the folder's contents are checked on disk by the
//! runner after Create worktree.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::dialog_guides;
use super::widgets::form_message::{self, Message};
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::PrefsMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::worktrees::{WorktreeForm, branch_problem, default_folder, folder_problem, folder_warning};
use crate::ui::theme::controls;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Id};
use glowfly_core::repo::Snapshot;
use glowfly_core::worktree_ops::WorktreeBase;

/// The folder's problem: git's refusal of it, or the runner's answer for this text.
fn folder_bad(form: &WorktreeForm, snap: Option<&Snapshot>) -> Option<String> {
    snap.and_then(|s| folder_problem(s, &form.folder)).or_else(|| form.current_error().map(str::to_owned))
}

/// The dialog for `form`; returns true when Cancel closes it (`update` closes it once the job
/// starts). The New branch field keeps the keyboard focus unless the folder field has it, or
/// `cx.keyboard` is false (a credential prompt is open above it).
pub fn new_worktree(ui: &mut egui::Ui, cx: Cx, form: &mut WorktreeForm, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let focus = cx.keyboard;
    let optional = cx.t.tr("dialog.field.optional");
    // The title, the branch field's help, and whether the branch is optional.
    let (heading, help, branch_optional) = match &form.base {
        WorktreeBase::Branch(b) => (format!("Create worktree from {b}"), format!("Leave empty to check out {b}"), true),
        WorktreeBase::Commit(_) => {
            (format!("Create worktree at {}", form.base.label()), "Leave empty for a detached HEAD".to_owned(), true)
        }
        WorktreeBase::Remote { .. } => {
            let full = form.base.label();
            (format!("Create worktree from {full}"), format!("Name of the new local branch; it tracks {full}"), false)
        }
    };
    let d = Dialog::new(cx, form.title.clone().unwrap_or(heading)).guide(dialog_guides::worktree_add(cx.t)).begin(ui);

    let folder_id = Id::new("glowfly-worktree-folder");
    let mut enter = false;
    let folder_was_bad = folder_bad(form, snap);
    let folder_msg = match &folder_was_bad {
        Some(p) => Some(Message::error(p.as_str())),
        None => snap.and_then(|s| folder_warning(s, &form.folder)).map(Message::warning),
    };
    let mut browse = false;
    let r = FieldGroup::new("Folder").message(&form.touched, "folder", folder_msg).show(ui, |ui| {
        let field = TextField::new(&mut form.folder).id(folder_id);
        let (r, b) = text_field::with_button(ui, field, buttons::secondary("Browse…"));
        browse = b.clicked();
        r
    });
    if r.changed() {
        form.folder_edited = true;
    }
    form_message::track(&mut form.touched, "folder", &r);
    enter |= text_field::entered(ui, &r);
    if browse {
        msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::WorktreeParent)));
    }

    let branch_was_bad = snap.and_then(|s| branch_problem(s, &form.base, &form.branch));
    let mut group = FieldGroup::new("New branch").help(help);
    if branch_optional {
        group = group.optional(optional);
    }
    // With no new branch the problem is the base's (checked out elsewhere): it shows at once.
    group = if form.branch.trim().is_empty() {
        group.note(branch_was_bad.as_deref().map(Message::error))
    } else {
        group.message(&form.touched, "branch", branch_was_bad.as_deref().map(Message::error))
    };
    let r = group.show(ui, |ui| ui.add(TextField::new(&mut form.branch).id(Id::new("glowfly-worktree-branch"))));
    if r.changed()
        && !form.folder_edited
        && let Some(s) = snap
    {
        form.folder = default_folder(s, &form.slug());
    }
    form_message::track(&mut form.touched, "branch", &r);
    // Read before focus is requested again below, which would hide that Enter left the field.
    enter |= text_field::entered(ui, &r);
    if focus && !ui.memory(|m| m.has_focus(folder_id)) {
        r.request_focus();
    }

    controls::checkbox(ui, &mut form.open, "Open in a new tab");
    // Checked again on the text as it is now, which this frame's typing may have changed.
    let branch_bad = snap.and_then(|s| branch_problem(s, &form.base, &form.branch));
    let valid = snap.is_some() && folder_bad(form, snap).is_none() && branch_bad.is_none() && !form.busy();
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Create worktree").enabled(valid)).submit(enter));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Worktree(WorktreeMsg::Submit));
    }
    out.dismiss
}
