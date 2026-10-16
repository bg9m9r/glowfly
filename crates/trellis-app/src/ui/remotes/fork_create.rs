//! The Fork dialog: looks for the user's own fork of a remote's repository first (offering to add
//! it), says when the repository is the user's own, and otherwise is the confirmation to fork it
//! into the user's account, add the fork as a remote and fetch it.

use super::super::theme::color;
use super::dialog::{field, note, problem_line};
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::forks::{ForkCreate, ForkStep, added_as, error_text};
use eframe::egui::{self, RichText};
use trellis_core::forge::Connection;
use trellis_core::remote_ops::{RemoteNameProblem, check_remote_name};
use trellis_core::repo::Snapshot;

fn spinner_line(ui: &mut egui::Ui, text: String) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new());
        ui.label(text);
    });
}

/// The remote name field and its problem line; returns (Enter pressed, the name is usable). The
/// field takes the focus when nothing has it, if `focus`.
fn name_field(ui: &mut egui::Ui, f: &mut ForkCreate, snap: Option<&Snapshot>, enabled: bool, focus: bool) -> (bool, bool) {
    let mut enter = false;
    ui.add_enabled_ui(enabled, |ui| {
        let (r, e) = field(ui, "Remote name", &mut f.name, "trellis-fork-create-name", "fork");
        if focus && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        enter = e;
    });
    let existing: Vec<&str> = snap.map(|s| s.remote_info.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
    let problem = check_remote_name(f.name.trim(), &existing, None).err();
    problem_line(ui, problem.as_ref().filter(|p| **p != RemoteNameProblem::Empty).map(|p| p.message()).unwrap_or_default());
    (enter, problem.is_none())
}

/// The Fork dialog; returns true when Cancel or Close closes it. The name field takes the focus
/// unless `focus` is false (a credential prompt is open above it). Add as remote is disabled
/// while a job runs (`idle` false), Fork and add remote while the fork is being created.
pub fn show(
    ui: &mut egui::Ui,
    f: &mut ForkCreate,
    snap: Option<&Snapshot>,
    connections: &[Connection],
    focus: bool,
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let repo = f.target.display();
    let service = f.target.provider.name();
    ui.heading(format!("Fork {repo} on {service}"));
    ui.add_space(8.0);
    // (primary button label, enabled, message) when the step has one.
    let mut primary: Option<(&str, bool, RemoteMsg)> = None;
    let mut submit = false;
    match f.step.clone() {
        ForkStep::Checking => match &f.error {
            None => spinner_line(ui, format!("Looking for your fork of {repo}…")),
            Some(e) => {
                ui.label(RichText::new(error_text(e)).color(color::red()));
                if ui.button("Retry").clicked() {
                    msgs.push(Msg::Remote(RemoteMsg::RetryForks));
                }
            }
        },
        ForkStep::Mine { fork, .. } => {
            ui.label(format!("You already have a fork of {repo}: {}.", fork.full_name));
            match snap.and_then(|s| added_as(s, &fork, connections)) {
                Some(r) => {
                    ui.add_space(4.0);
                    note(ui, format!("It is already the remote {r}."));
                }
                None => {
                    ui.add_space(6.0);
                    let (enter, ok) = name_field(ui, f, snap, true, focus);
                    ui.add_space(4.0);
                    ui.checkbox(&mut f.fetch_after, "Fetch its branches after adding");
                    submit = enter;
                    primary = Some(("Add as remote", ok && idle, RemoteMsg::AddMyFork));
                }
            }
        }
        ForkStep::OwnRepo { user } => {
            ui.label(format!("{repo} belongs to your account ({user}), so there is nothing to fork."));
        }
        ForkStep::Ready { user } | ForkStep::Creating { user } => {
            let creating = matches!(f.step, ForkStep::Creating { .. });
            ui.label(format!("Creates a copy of {repo} under your account ({user}) on {service}, adds it as a remote and fetches it."));
            ui.add_space(6.0);
            let (enter, ok) = name_field(ui, f, snap, !creating, focus);
            if let Some(e) = &f.error {
                ui.label(RichText::new(error_text(e)).color(color::red()));
            }
            if creating {
                ui.add_space(4.0);
                spinner_line(ui, format!("Creating the fork on {service}… This can take a few seconds."));
            }
            submit = enter;
            primary = Some(("Fork and add remote", ok && !creating, RemoteMsg::CreateFork));
        }
    }
    let mut close = false;
    ui.add_space(10.0);
    ui.horizontal(|ui| match primary {
        Some((label, enabled, msg)) => {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() || (submit && enabled) {
                msgs.push(Msg::Remote(msg));
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        }
        None => {
            let label = if matches!(f.step, ForkStep::Checking) { "Cancel" } else { "Close" };
            if ui.button(label).clicked() {
                close = true;
            }
        }
    });
    close
}
