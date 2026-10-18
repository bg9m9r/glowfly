//! The Reword dialog (`Modal::RewordCommit`): a new message for a commit chosen in the graph,
//! or, as the Squash dialog, the message of the commit a squash makes (pre-filled with the
//! joined messages). It says which commits the rebase rewrites and, on a dirty tree, that the
//! changes are stashed first; Ctrl/Cmd+Enter submits. `update` checks the form again before running anything.

use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::needs_stash;
use crate::state::rebase_quick::{BLANK, RewordForm, STASH_NOTE};
use crate::ui::theme::color;
use eframe::egui::{self, Color32, Key, Modifiers, RichText, Stroke};
use trellis_core::repo::Snapshot;

/// Draws the dialog; returns true when it should close (Cancel). The message field takes the
/// keyboard focus once, unless `focus` is false (a credential prompt is open above it). The
/// primary button waits while a job runs (`idle` false); on submit the dialog stays open until
/// `update` starts the rebase.
pub fn show(ui: &mut egui::Ui, form: &mut RewordForm, snap: Option<&Snapshot>, focus: bool, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(form.title());
    ui.label(RichText::new(form.subtitle()).color(color::weak()));
    ui.add_space(6.0);
    // Read before the field sees the key, so the Enter doesn't become a new line.
    let submit = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
    let hint = form.hint();
    let r = ui.add(egui::TextEdit::multiline(&mut form.draft).hint_text(hint).desired_rows(6).desired_width(f32::INFINITY));
    if focus && std::mem::take(&mut form.focus) {
        r.request_focus();
    }
    if form.draft.trim().is_empty() {
        ui.label(RichText::new(BLANK).color(color::red()));
    }
    let stash = snap.is_some_and(needs_stash);
    if stash {
        ui.add_space(4.0);
        ui.label(RichText::new(STASH_NOTE).color(color::weak()));
    }
    ui.add_space(10.0);
    let blocked = form.blocker(idle);
    ui.horizontal(|ui| {
        let enabled = blocked.is_none();
        let (fg, fill) = if enabled { (Color32::WHITE, color::primary()) } else { (color::faint(), color::primary_off()) };
        let text = form.button(stash);
        let button =
            egui::Button::new(RichText::new(text).strong().color(fg)).fill(fill).stroke(Stroke::new(1.0_f32, color::primary_border()));
        let mut r = ui.add_enabled(enabled, button);
        if let Some(why) = blocked {
            r = r.on_disabled_hover_text(why);
        }
        if r.clicked() || (submit && enabled) {
            msgs.push(Msg::Rebase(RebaseMsg::SubmitReword));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
