//! The Create worktree dialog: a folder (typed, or a parent picked with Browse…), an optional new
//! branch and whether to open the worktree in a tab. Problems are checked as the user types with
//! the pure rules in `state::worktrees`; the folder's contents are checked on disk by the runner
//! after Create worktree.

use super::theme::color;
use crate::msg::PrefsMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::worktrees::{WorktreeForm, branch_problem, default_folder, folder_problem, folder_warning};
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::repo::Snapshot;
use trellis_core::worktree_ops::WorktreeBase;

/// The dialog for `form`; returns true when Cancel closes it (`update` closes it once the job
/// starts). The New branch field keeps the keyboard focus unless the folder field has it, or
/// `focus` is false (a credential prompt is open above it).
pub fn new_worktree(ui: &mut egui::Ui, form: &mut WorktreeForm, snap: Option<&Snapshot>, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    let (heading, hint) = match &form.base {
        WorktreeBase::Branch(b) => (format!("Create worktree from {b}"), format!("Optional. Leave empty to check out {b}")),
        WorktreeBase::Commit(_) => {
            (format!("Create worktree at {}", form.base.label()), "Optional. Leave empty for a detached HEAD".to_owned())
        }
    };
    ui.heading(heading);
    ui.add_space(6.0);

    let folder_id = Id::new("trellis-worktree-folder");
    ui.label(RichText::new("Folder").color(color::weak()));
    let mut enter = false;
    ui.horizontal(|ui| {
        let browse_w = 80.0;
        let r = ui.add(egui::TextEdit::singleline(&mut form.folder).id(folder_id).desired_width(ui.available_width() - browse_w));
        if r.changed() {
            form.folder_edited = true;
        }
        enter |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if ui.button("Browse…").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::WorktreeParent)));
        }
    });
    let folder_bad = snap.and_then(|s| folder_problem(s, &form.folder)).or_else(|| form.current_error().map(str::to_owned));
    if let Some(p) = &folder_bad {
        ui.label(RichText::new(p).color(color::red()));
    } else if let Some(w) = snap.and_then(|s| folder_warning(s, &form.folder)) {
        ui.label(RichText::new(w).color(color::amber()));
    }

    ui.add_space(6.0);
    ui.label(RichText::new("New branch").color(color::weak()));
    let r = ui.add(
        egui::TextEdit::singleline(&mut form.branch).id(Id::new("trellis-worktree-branch")).hint_text(hint).desired_width(f32::INFINITY),
    );
    if r.changed()
        && !form.folder_edited
        && let Some(s) = snap
    {
        form.folder = default_folder(s, &form.slug());
    }
    // Read before focus is requested again below, which would hide that Enter left the field.
    enter |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus && !ui.memory(|m| m.has_focus(folder_id)) {
        r.request_focus();
    }
    let branch_bad = snap.and_then(|s| branch_problem(s, &form.base, &form.branch));
    if let Some(p) = &branch_bad {
        ui.label(RichText::new(p).color(color::red()));
    }

    ui.add_space(4.0);
    ui.checkbox(&mut form.open, "Open in a new tab");
    ui.add_space(8.0);
    let valid = snap.is_some() && folder_bad.is_none() && branch_bad.is_none() && !form.busy();
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Create worktree")).clicked() || (enter && valid) {
            msgs.push(Msg::Worktree(WorktreeMsg::Submit));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
