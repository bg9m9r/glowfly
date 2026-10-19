//! The Git Flow Finish dialog: the branch, the tag name and message (release and hotfix), Keep
//! the branch, and a live check against the snapshot: the numbered steps Finish will run, or
//! the problem stopping it.

use super::common::capitalize;
use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::buttons;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::gitflow::FinishForm;
use crate::ui::theme::controls;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, Key, RichText};
use trellis_core::gitflow::{Branches, FinishProblem, WorkState, check_finish, short_name};
use trellis_core::repo::Snapshot;

/// Draws the dialog; returns true when it should close. `cx.keyboard` is false under a
/// credential prompt, whose fields need the keyboard. Ctrl/Cmd+Enter in the message finishes.
pub fn finish(ui: &mut egui::Ui, cx: Cx, form: &mut FinishForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let Some((snap, s)) = snap.and_then(|snap| Some((snap, snap.gitflow.settings()?))) else { return true };
    let kind = form.kind;
    let focus = cx.keyboard;
    let title = format!("Finish {} {}", kind.noun(), short_name(&s, kind, &form.branch));
    let d = Dialog::new(cx, title).subtitle(format!("Branch {}", form.branch)).begin(ui);
    let mut submit = false;
    if kind.versioned() {
        let hint = trellis_core::gitflow::default_tag(&s, kind, &form.branch).unwrap_or_default();
        egui::Grid::new("gitflow-finish-grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Tag");
            let r = controls::field(ui, egui::TextEdit::singleline(&mut form.tag).hint_text(hint).desired_width(f32::INFINITY));
            if focus && form.focus_tag {
                r.request_focus();
                form.focus_tag = false;
            }
            submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.end_row();
            ui.label("Message");
            controls::field(ui, egui::TextEdit::multiline(&mut form.message).desired_rows(3).desired_width(f32::INFINITY));
            ui.end_row();
        });
        ui.add_space(4.0);
    }
    controls::checkbox(ui, &mut form.keep, format!("Keep {} after finishing", form.branch));
    ui.add_space(4.0);
    let result = check_finish(&s, &form.branch, &form.options(), &Branches::of(snap), &WorkState::of(snap));
    match &result {
        Ok(plan) => {
            ui.label(RichText::new("Steps:").color(color::weak()));
            for (i, step) in plan.steps.iter().enumerate() {
                ui.label(RichText::new(format!("{}. {}", i + 1, capitalize(&step.what))).color(color::weak()));
            }
        }
        // An empty tag or message only disables Finish.
        Err(FinishProblem::EmptyTag | FinishProblem::EmptyMessage) => {}
        Err(p) => {
            form_message::line(ui, Message::error(p.message()));
        }
    }
    let valid = result.is_ok() && idle;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Finish").enabled(valid)).submit(submit));
    // Update closes the dialog once the job starts.
    if out.primary {
        msgs.push(Msg::GitFlow(GitFlowMsg::Finish { branch: form.branch.clone(), options: form.options() }));
    }
    out.dismiss
}
