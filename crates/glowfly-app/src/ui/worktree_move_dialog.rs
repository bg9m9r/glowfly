//! The Move worktree dialog: the new folder (typed, or a parent picked with Browse…), checked as
//! the user types with the pure rules in `state::worktree_move`; the runner checks the disk and
//! reports a taken folder under the field. A side pane explains the step. `update` closes the
//! dialog once the job starts.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::dialog_guides;
use super::widgets::form_message::{self, Message};
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::{FolderPurpose, Msg, PrefsMsg};
use crate::state::worktree_move::{MoveForm, UNMOVED, target_problem};
use crate::state::worktrees::folder_warning;
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Id};
use glowfly_core::repo::Snapshot;

/// The dialog for `form`; returns true when Cancel closes it. The folder field has the keyboard
/// focus unless `cx.keyboard` is false (a credential prompt is open above it); Enter moves.
pub fn move_worktree(ui: &mut egui::Ui, cx: Cx, form: &mut MoveForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let focus = cx.keyboard;
    let d = Dialog::new(cx, format!("Move worktree {}", form.name)).guide(dialog_guides::worktree_move(cx.t)).begin(ui);
    let bad = problem(form, snap);
    if form.current_error().is_some() {
        // The runner refused the last submit: its error shows like any other problem.
        form.touched.attempt();
    }
    let message = match &bad {
        // The field starts on the current folder: say so, as information.
        Some(p) if p == UNMOVED => Some(Message::info(UNMOVED)),
        Some(p) => Some(Message::error(p.as_str())),
        None => snap.and_then(|s| folder_warning(s, &form.folder)).map(Message::warning),
    };
    let mut browse = false;
    let group =
        FieldGroup::new("New folder").message(&form.touched, "folder", message).help("A tab showing it follows it to the new folder.");
    let r = group.show(ui, |ui| {
        let field = TextField::new(&mut form.folder).id(Id::new("trellis-worktree-move-folder"));
        let (r, b) = text_field::with_button(ui, field, buttons::secondary("Browse…"));
        browse = b.clicked();
        r
    });
    form_message::track(&mut form.touched, "folder", &r);
    let enter = text_field::entered(ui, &r);
    if focus && !enter {
        r.request_focus();
    }
    if browse {
        msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::WorktreeParent)));
    }
    // Checked again on the text as it is now, which this frame's typing may have changed.
    let valid = snap.is_some() && problem(form, snap).is_none() && idle;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Move").enabled(valid)).submit(enter));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Worktree(WorktreeMsg::Move));
    }
    out.dismiss
}

/// The new folder's problem: the rules' or the runner's answer for this text.
fn problem(form: &MoveForm, snap: Option<&Snapshot>) -> Option<String> {
    snap.and_then(|s| target_problem(s, &form.path, &form.folder)).or_else(|| form.current_error().map(str::to_owned))
}
