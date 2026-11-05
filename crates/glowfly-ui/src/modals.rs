//! Dialogs, every one drawn through `dialog_frame` (header with its close button, which sends
//! `Dismiss::CloseButton` like Esc, scrolling body, footer in the fixed order, Enter and the
//! default button's focus, and for less familiar operations a side pane explaining them) in
//! `theme::dialog`: confirmation (drawn by `confirm_dialog.rs`, also in its danger form and with an alternative answer), new branch, new tag and annotate tag (drawn by `tag_dialog.rs`), create
//! worktree (drawn by `worktree_dialog.rs`), remove and lock worktree (drawn by
//! `worktree_manage_dialog.rs`), add and remove submodule (drawn by `submodule_dialogs.rs`), clone
//! (drawn by `clone_dialog.rs` and `clone_service.rs`), init (drawn by `init_dialog.rs`), create
//! pull request (drawn by `pull_request/create_dialog.rs`), publish (drawn by `publish.rs`), renaming a
//! branch or setting its upstream (drawn by `branch_dialogs.rs`), renaming a stash (drawn by
//! `stash_dialog.rs`), stashing with a message and a choice of what goes in (drawn by
//! `stash_push_dialog.rs`), creating or renaming a working-tree file (drawn by `file_dialogs.rs`),
//! starting a Git Flow branch (drawn by `gitflow_dialog.rs`), finishing one (drawn by
//! `gitflow_finish_dialog.rs`), adding or editing a remote (drawn by `remotes/dialog.rs`), adding a
//! fork as a remote and forking on the hosting service (drawn by `remotes/fork_pick.rs` and
//! `remotes/fork_create.rs`), the keyboard shortcut sheet (drawn by `shortcuts/sheet.rs`), the
//! repository trust prompt (drawn by `trust_dialog.rs`), a conflict check's report (drawn by
//! `conflict_check.rs`), what an operation would do before it runs (drawn by
//! `op_preview_dialog.rs`), a pull request or issue filter (drawn by `forge_filter_dialog.rs`), the
//! glowfly:// link dialogs (drawn by `deep_link_dialog.rs`), creating an issue (drawn by
//! `issue/create_dialog.rs`), rewording a commit from the graph (drawn by
//! `rebase/reword_dialog.rs`), the About dialog and its third-party notices (drawn by `menu_bar/about.rs` and
//! `menu_bar/notices.rs`), the release notes
//! (drawn by `release_notes.rs`), undoing a checkout that created a branch (drawn by
//! `undo_dialog.rs`), applying a patch file (drawn by `patch_apply_dialog.rs`), creating a patch
//! from uncommitted changes (drawn by `patch_save_dialog.rs`), finding repositories in project
//! folders (drawn by `repo_finder.rs`), fetching or pulling across repositories (drawn by
//! `repo_sync.rs`), renaming a tab (drawn by `tab_dialog.rs`), adding repositories to a group
//! (drawn by `repo_group_picker.rs`), choosing a merge's mainline parent
//! (drawn by `mainline_dialog.rs`), the home Trash offer (drawn by `trash_offer_dialog.rs`),
//! stashing uncommitted changes before an operation (drawn by `auto_stash_dialog.rs`) and
//! generating an SSH key (drawn by `prefs/ssh/keygen_dialog.rs`). An edited form kept under a question
//! (state/dialog_guard.rs) is drawn first, beneath the question.
//!
//! A form is drawn from the frame's copy (`modal_forms.rs`), sent back as `EditMsg::Dialog`
//! when it changed; the other dialogs are drawn from the dialog itself (`modal_notes.rs`).

use crate::dialog_frame::Cx;
use eframe::egui;
use glowfly_model::msg::{DialogMsg, Dismiss, EditMsg, Msg};
use glowfly_model::state::modal_form::DialogForm;
use glowfly_model::state::{AppState, Modal, ViewState};

#[path = "confirm_dialog.rs"]
mod confirm_dialog;
#[path = "modal_forms.rs"]
mod modal_forms;
#[path = "modal_notes.rs"]
mod modal_notes;

pub fn show(ctx: &egui::Context, st: &AppState, view: &mut ViewState, msgs: &mut Vec<Msg>) {
    // An edited form kept under a question (state/dialog_guard.rs) is drawn first, beneath it:
    // dimmed, without the keyboard, and what it sends (its edits too) is dropped.
    let over = match st.dialog.under() {
        Some(form) => {
            draw(ctx, st, view, form, &mut Vec::new(), Layer::Under);
            true
        }
        None => false,
    };
    if let Some(shown) = st.modal.as_ref() {
        draw(ctx, st, view, shown, msgs, if over { Layer::Over } else { Layer::Only });
    }
}

/// Where the dialog drawn stacks.
#[derive(Clone, Copy, PartialEq)]
enum Layer {
    /// The one dialog.
    Only,
    /// The form kept under a question.
    Under,
    /// The question over a form kept underneath.
    Over,
}

fn draw(ctx: &egui::Context, st: &AppState, view: &mut ViewState, shown: &Modal, msgs: &mut Vec<Msg>, layer: Layer) {
    // Under a credential prompt, Esc and clicks belong to the prompt.
    let prompting = st.askpass.showing();
    // Under the "Discard your changes?" question, the form's fields don't take the focus.
    let asking = st.dialog.asking && layer != Layer::Under;
    let keyboard = !prompting && !asking && layer != Layer::Under;
    // The form underneath keeps the usual id, so its fields keep their state when it is back.
    let area = if layer == Layer::Over { "glowfly-modal-over" } else { "glowfly-modal" };
    let env = modal_forms::DialogEnv::new(st, shown);
    let t = env.t;
    // The form's copy for this frame (None for a dialog that only reads).
    let mut copy = shown.form_copy();
    let mut close = false;
    let first = msgs.len();
    let resp = super::theme::show_modal(ctx, super::theme::dialog(ctx, egui::Id::new(area)), |ui| {
        // A dialog's key in egui's memory: the first frame of each kind takes the focus.
        let cx = Cx { t, keyboard, key: egui::Id::new("glowfly-dialog").with(std::mem::discriminant(shown)) };
        close = match copy.as_mut() {
            Some(form) => modal_forms::draw(ui, cx, form, &env, msgs),
            None => modal_notes::draw(ui, cx, shown, &env, view, msgs),
        };
    });
    // Taken every frame, so a click that closed this dialog never reaches the next one.
    let closed = super::dialog_frame::take_closed(&resp);
    if layer == Layer::Under {
        return;
    }
    if let Some(form) = copy
        && !shown.same_form(&form)
    {
        msgs.push(Msg::Edit(EditMsg::Dialog(DialogForm::new(form))));
    }
    if close {
        // The dialog closes before what it asked for runs, so a dialog that opens (the trust
        // prompt holding the job it started) isn't closed with it.
        msgs.insert(first, Msg::CloseModal);
    } else if asking {
        // Drawn after the form, so it stacks on top and the form beneath takes no input.
        super::dialog_guard::discard_question(ctx, t, msgs);
    } else if !prompting && let Some(how) = if closed { Some(Dismiss::CloseButton) } else { super::dialog_guard::dismissal(&resp) } {
        msgs.insert(first, Msg::Dialog(DialogMsg::Dismiss(how)));
    }
}
