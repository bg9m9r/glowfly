//! The Move worktree dialog: the new folder (typed, or a parent picked with Browse…), checked as
//! the user types with the pure rules in `state::worktree_move`; the runner checks the disk and
//! reports a taken folder under the field. `update` closes the dialog once the job starts.

use super::theme::color;
use crate::msg::{FolderPurpose, Msg, PrefsMsg};
use crate::state::worktree_move::{MoveForm, target_problem};
use crate::state::worktrees::folder_warning;
use crate::ui::theme::controls;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::repo::Snapshot;

/// The dialog for `form`; returns true when Cancel closes it. The folder field has the keyboard
/// focus unless `focus` is false (a credential prompt is open above it); Enter moves.
pub fn move_worktree(
    ui: &mut egui::Ui,
    form: &mut MoveForm,
    snap: Option<&Snapshot>,
    focus: bool,
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let mut close = false;
    ui.heading(format!("Move worktree {}", form.name));
    ui.add_space(6.0);
    ui.label(RichText::new("New folder").color(color::weak()));
    let mut enter = false;
    ui.horizontal(|ui| {
        let browse_w = 80.0;
        let edit = egui::TextEdit::singleline(&mut form.folder).id(Id::new("trellis-worktree-move-folder"));
        let r = controls::field(ui, edit.desired_width(ui.available_width() - browse_w));
        enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if focus && !enter {
            r.request_focus();
        }
        if ui.button("Browse…").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::WorktreeParent)));
        }
    });
    let bad = snap.and_then(|s| target_problem(s, &form.path, &form.folder)).or_else(|| form.current_error().map(str::to_owned));
    if let Some(p) = &bad {
        ui.label(RichText::new(p).color(color::red()));
    } else if let Some(w) = snap.and_then(|s| folder_warning(s, &form.folder)) {
        ui.label(RichText::new(w).color(color::amber()));
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new("git moves the folder and updates its records. A tab showing it follows it to the new folder.").color(color::weak()),
    );
    ui.add_space(10.0);
    let valid = snap.is_some() && bad.is_none() && idle;
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Move")).clicked() || (enter && valid) {
            msgs.push(Msg::Worktree(WorktreeMsg::Move));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
