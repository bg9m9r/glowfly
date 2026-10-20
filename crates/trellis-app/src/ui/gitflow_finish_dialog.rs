//! The Git Flow Finish dialog: the branch, the tag name and message (release and hotfix), Keep
//! the branch, and a live check against the snapshot: the numbered steps Finish will run, or
//! the problem stopping it.

use super::common::capitalize;
use super::dialog_frame::{Cx, Dialog, Footer};
use super::dialog_guides;
use super::theme::color;
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::gitflow::FinishForm;
use crate::ui::theme::controls;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText};
use trellis_core::gitflow::{Branches, FinishProblem, WorkState, check_finish, short_name};
use trellis_core::repo::Snapshot;

/// Draws the dialog; returns true when it should close. `cx.keyboard` is false under a
/// credential prompt, whose fields need the keyboard. Ctrl/Cmd+Enter in the message finishes.
pub fn finish(ui: &mut egui::Ui, cx: Cx, form: &mut FinishForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let Some((snap, s)) = snap.and_then(|snap| Some((snap, snap.gitflow.settings()?))) else { return true };
    let kind = form.kind;
    let focus = cx.keyboard;
    let title = format!("Finish {} {}", kind.noun(), short_name(&s, kind, &form.branch));
    let d = Dialog::new(cx, title).subtitle(format!("Branch {}", form.branch)).guide(dialog_guides::gitflow_finish(cx.t)).begin(ui);
    let mut submit = false;
    if kind.versioned() {
        let hint = trellis_core::gitflow::default_tag(&s, kind, &form.branch).unwrap_or_default();
        // The tag's own problem, under it, once it was edited or Finish was tried.
        let checked = check_finish(&s, &form.branch, &form.options(), &Branches::of(snap), &WorkState::of(snap));
        let shown = checked.err().filter(tag_problem).map(|p| Message::error(p.message()));
        let group = FieldGroup::new("Tag").message(&form.touched, "tag", shown);
        let r = group.show(ui, |ui| ui.add(TextField::new(&mut form.tag).hint(hint)));
        form_message::track(&mut form.touched, "tag", &r);
        if focus && form.focus_tag {
            r.request_focus();
            form.focus_tag = false;
        }
        submit = text_field::entered(ui, &r);
        FieldGroup::new("Tag message").show(ui, |ui| ui.add(TextField::multiline(&mut form.message, 3)));
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
        // An empty tag or message only disables Finish; the tag's problem is under it.
        Err(FinishProblem::EmptyTag | FinishProblem::EmptyMessage) => {}
        Err(p) if tag_problem(p) => {}
        // The repository's state: it shows at once.
        Err(p) => form_message::form(ui, Message::error(p.message())),
    }
    let valid = result.is_ok() && idle;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Finish").enabled(valid)).submit(submit));
    if out.attempted {
        form.touched.attempt();
    }
    // Update closes the dialog once the job starts.
    if out.primary {
        msgs.push(Msg::GitFlow(GitFlowMsg::Finish { branch: form.branch.clone(), options: form.options() }));
    }
    out.dismiss
}

/// Whether `p` is about the tag typed (not the repository's state).
fn tag_problem(p: &FinishProblem) -> bool {
    matches!(p, FinishProblem::InvalidTag(_) | FinishProblem::TagExists(_) | FinishProblem::TagClashes(..) | FinishProblem::TagUnusable(_))
}
