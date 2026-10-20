//! The Git Flow Start dialog: the kind's prefix, a name field (prefilled with the next version
//! for a release or hotfix), what Start will do, and any problem with the name.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::dialog_guides;
use super::theme::color;
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::gitflow::StartForm;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText};
use trellis_core::branch_ops::NameProblem;
use trellis_core::gitflow::{Branches, StartProblem, check_start, open_branches};
use trellis_core::repo::Snapshot;

/// Draws the dialog; returns true when it should close.
pub fn start(ui: &mut egui::Ui, cx: Cx, form: &mut StartForm, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let Some((snap, s)) = snap.and_then(|snap| Some((snap, snap.gitflow.settings()?))) else { return true };
    let b = Branches::of(snap);
    let kind = form.kind;
    let d = Dialog::new(cx, format!("Start a {}", kind.noun())).guide(dialog_guides::gitflow_start(cx.t)).begin(ui);
    let hint = if kind.versioned() { "1.4.0" } else { "short-name" };
    // What Start will do goes under the field until the name has a problem.
    let (help, shown) = match check_start(&s, kind, form.name.trim(), &b) {
        Ok(branch) => (Some(format!("Creates {branch} from {} and checks it out.", s.base(kind))), None),
        // An empty name only disables Start; the base's problem shows above the footer.
        Err(StartProblem::Name(NameProblem::Empty, _) | StartProblem::MissingBase { .. }) => (None, None),
        Err(p) => (None, Some(Message::error(p.message()))),
    };
    let mut group = FieldGroup::new("Name").message(&form.touched, "name", shown);
    if let Some(help) = help {
        group = group.help(help);
    }
    let prefix = s.prefix(kind);
    let r = group.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(prefix).color(color::weak()));
            ui.add(TextField::new(&mut form.name).hint(hint))
        })
        .inner
    });
    form_message::track(&mut form.touched, "name", &r);
    let submit = text_field::entered(ui, &r);
    // Not under a credential prompt: its fields need the keyboard.
    if cx.keyboard {
        r.request_focus();
    }
    let name = form.name.trim();
    let result = check_start(&s, kind, name, &b);
    // The repository's state, not the name: it shows at once.
    if let Err(p @ StartProblem::MissingBase { .. }) = &result {
        form_message::form(ui, Message::error(p.message()));
    }
    if kind.versioned() {
        let open = open_branches(&s, kind, &b);
        if !open.is_empty() {
            let verb = if open.len() == 1 { "is" } else { "are" };
            ui.label(RichText::new(format!("{} {verb} still open.", open.join(", "))).color(color::weak()));
        }
    }
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Start").enabled(result.is_ok())).submit(submit));
    if out.attempted {
        form.touched.attempt();
    }
    // Update closes the dialog once the job starts.
    if out.primary {
        msgs.push(Msg::GitFlow(GitFlowMsg::Start { kind, name: name.to_owned() }));
    }
    out.dismiss
}
