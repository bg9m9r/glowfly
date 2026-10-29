//! The Fork dialog: looks for the user's own fork of a remote's repository first (offering to add
//! it), says when the repository is the user's own, and otherwise is the confirmation to fork it
//! into the user's account, add the fork as a remote and fetch it. Opened from the create pull
//! request dialog (`then_pr`), it also pushes the branch to the fork.

use super::super::dialog_frame::{Cx, Dialog, Footer};
use super::super::pane_status::{self, Failure};
use super::super::widgets::form_message::{self, Message};
use super::super::widgets::{FieldGroup, buttons};
use super::dialog::{field, note, problem};
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::blocked::BUSY;
use crate::state::forks::{ForkCreate, ForkStep, added_as, error_text};
use crate::state::touched::Touched;
use crate::ui::theme::controls;
use eframe::egui;
use glowfly_core::forge::Connection;
use glowfly_core::remote_ops::{RemoteNameProblem, check_remote_name};
use glowfly_core::repo::Snapshot;

fn spinner_line(ui: &mut egui::Ui, text: String) {
    pane_status::loading_line(ui, &text);
}

/// Why the primary is disabled for an empty remote name (never shown under the field).
pub(super) const NO_NAME: &str = "Enter a name for the remote.";

/// What the footer says about the remote name's `problem`: an empty name always, any other
/// problem only until the field shows it (once touched or after a try).
pub(super) fn name_why(problem: Option<&RemoteNameProblem>, touched: &Touched) -> Option<String> {
    match problem? {
        RemoteNameProblem::Empty => Some(NO_NAME.to_owned()),
        p if !touched.shows("name") => Some(p.message()),
        _ => None,
    }
}

/// The remote name field and its problem line; returns whether Enter was pressed and the name's
/// problem, if any. The field takes the focus when nothing has it, if `focus`.
fn name_field(
    ui: &mut egui::Ui,
    f: &mut ForkCreate,
    snap: Option<&Snapshot>,
    enabled: bool,
    focus: bool,
) -> (bool, Option<RemoteNameProblem>) {
    let mut enter = false;
    let existing: Vec<&str> = snap.map(|s| s.remote_info.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
    let check = |name: &str| check_remote_name(name.trim(), &existing, None).err();
    let shown = check(&f.name).filter(|p| *p != RemoteNameProblem::Empty).and_then(|p| problem(p.message()));
    ui.add_enabled_ui(enabled, |ui| {
        let group = FieldGroup::new("Remote name").message(&f.touched, "name", shown);
        let (r, e) = field(ui, group, &mut f.name, "glowfly-fork-create-name", "fork");
        form_message::track(&mut f.touched, "name", &r);
        if focus && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        enter = e;
    });
    (enter, check(&f.name))
}

/// The Fork dialog; returns true when Cancel or Close closes it. The name field takes the focus
/// unless `focus` is false (a credential prompt is open above it). Add as remote is disabled
/// while a job runs (`idle` false), Fork and add remote while the fork is being created.
pub fn show(
    ui: &mut egui::Ui,
    cx: Cx,
    f: &mut ForkCreate,
    snap: Option<&Snapshot>,
    connections: &[Connection],
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let focus = cx.keyboard;
    let repo = f.target.display();
    let service = f.target.provider.name();
    let d = Dialog::new(cx, format!("Fork {repo} on {service}")).begin(ui);
    // (primary button label, enabled, why not, message) when the step has one.
    let mut primary: Option<(String, bool, Option<String>, RemoteMsg)> = None;
    let mut submit = false;
    let then = f.then_pr.as_ref().map(|t| t.branch.clone());
    if let Some(branch) = &then {
        let noun = f.target.provider.pr_noun();
        note(ui, format!("Then pushes {branch} to the fork and opens the Create {noun} dialog."));
        ui.add_space(6.0);
    }
    match f.step.clone() {
        ForkStep::Checking => match &f.error {
            None => spinner_line(ui, format!("Looking for your fork of {repo}…")),
            Some(e) => {
                let error = error_text(cx.t, &f.target, e);
                let f = Failure { title: "Couldn't look for your fork", error: &error, retry: Some(Msg::Remote(RemoteMsg::RetryForks)) };
                pane_status::failed_line(ui, cx.t, f, msgs);
            }
        },
        ForkStep::Mine { fork, .. } => {
            ui.label(format!("You already have a fork of {repo}: {}.", fork.full_name));
            match snap.and_then(|s| added_as(s, &fork, connections)) {
                Some(r) => {
                    ui.add_space(4.0);
                    note(ui, format!("It is already the remote {r}."));
                    if let Some(branch) = &then {
                        let why = (!idle).then(|| BUSY.to_owned());
                        primary = Some((format!("Push {branch} to {r}"), idle, why, RemoteMsg::AddMyFork));
                    }
                }
                None => {
                    ui.add_space(6.0);
                    let (enter, problem) = name_field(ui, f, snap, true, focus);
                    ui.add_space(4.0);
                    controls::checkbox(ui, &mut f.fetch_after, "Fetch its branches after adding");
                    submit = enter;
                    let why = if idle { name_why(problem.as_ref(), &f.touched) } else { Some(BUSY.to_owned()) };
                    let label = if then.is_some() { "Add as remote and push" } else { "Add as remote" };
                    primary = Some((label.to_owned(), problem.is_none() && idle, why, RemoteMsg::AddMyFork));
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
            let (enter, problem) = name_field(ui, f, snap, !creating, focus);
            if let Some(e) = &f.error {
                ui.add_space(4.0);
                form_message::form(ui, Message::error(error_text(cx.t, &f.target, e)));
            }
            if creating {
                ui.add_space(4.0);
                spinner_line(ui, format!("Creating the fork on {service}… This can take a few seconds."));
            }
            submit = enter;
            // While creating, the spinner says why.
            let why = if creating { None } else { name_why(problem.as_ref(), &f.touched) };
            let label = if then.is_some() { "Fork, add remote and push" } else { "Fork and add remote" };
            primary = Some((label.to_owned(), problem.is_none() && !creating, why, RemoteMsg::CreateFork));
        }
    }
    let footer = match &primary {
        Some((label, enabled, why, _)) => {
            Footer::new().cancel().primary(buttons::primary(label.as_str()).enabled(*enabled).why(why.as_deref())).submit(submit)
        }
        // While it looks for the fork, nothing has happened yet to close.
        None if matches!(f.step, ForkStep::Checking) => Footer::new().cancel(),
        None => Footer::new().close(),
    };
    let out = d.finish(ui, footer);
    if out.attempted {
        f.touched.attempt();
    }
    if out.primary
        && let Some((_, _, _, msg)) = primary
    {
        msgs.push(Msg::Remote(msg));
    }
    out.dismiss
}
