//! The Git Flow Start dialog: the kind's prefix, a name field (prefilled with the next version
//! for a release or hotfix), what Start will do, and any problem with the name.

use super::theme::color;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::gitflow::StartForm;
use crate::ui::theme::controls;
use eframe::egui::{self, Key, RichText};
use trellis_core::branch_ops::NameProblem;
use trellis_core::gitflow::{Branches, StartProblem, check_start, open_branches};
use trellis_core::repo::Snapshot;

/// Draws the dialog; returns true when it should close.
pub fn start(ui: &mut egui::Ui, form: &mut StartForm, snap: Option<&Snapshot>, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let Some((snap, s)) = snap.and_then(|snap| Some((snap, snap.gitflow.settings()?))) else { return true };
    let b = Branches::of(snap);
    let kind = form.kind;
    let mut close = false;
    ui.heading(format!("Start a {}", kind.noun()));
    ui.add_space(6.0);
    let hint = if kind.versioned() { "1.4.0" } else { "short-name" };
    let r = ui
        .horizontal(|ui| {
            ui.label(RichText::new(s.prefix(kind)).color(color::weak()));
            controls::field(ui, egui::TextEdit::singleline(&mut form.name).hint_text(hint).desired_width(f32::INFINITY))
        })
        .inner;
    // Not under a credential prompt: its fields need the keyboard.
    if focus {
        r.request_focus();
    }
    let name = form.name.trim();
    let result = check_start(&s, kind, name, &b);
    ui.add_space(4.0);
    match &result {
        Ok(branch) => {
            let line = format!("Creates {branch} from {} and checks it out.", s.base(kind));
            ui.label(RichText::new(line).color(color::weak()));
        }
        // An empty name only disables Start.
        Err(StartProblem::Name(NameProblem::Empty, _)) => {}
        Err(p) => {
            ui.label(RichText::new(p.message()).color(color::red()));
        }
    }
    if kind.versioned() {
        let open = open_branches(&s, kind, &b);
        if !open.is_empty() {
            let verb = if open.len() == 1 { "is" } else { "are" };
            ui.label(RichText::new(format!("{} {verb} still open.", open.join(", "))).color(color::weak()));
        }
    }
    ui.add_space(10.0);
    let valid = result.is_ok();
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    ui.horizontal(|ui| {
        // Update closes the dialog once the job starts.
        if ui.add_enabled(valid, egui::Button::new("Start")).clicked() || (submit && valid) {
            msgs.push(Msg::GitFlow(GitFlowMsg::Start { kind, name: name.to_owned() }));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
