//! The Fork dialog: looks for the user's own fork of a remote's repository first (offering to add
//! it), says when the repository is the user's own, and otherwise is the confirmation to fork it
//! into the user's account, add the fork as a remote and fetch it.

use super::super::dialog_frame::{Cx, Dialog, Footer};
use super::super::pane_status::{self, Failure};
use super::super::widgets::buttons;
use super::super::widgets::form_message::{self, Message};
use super::dialog::{field, note, problem_line};
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::forks::{ForkCreate, ForkStep, added_as, error_text};
use crate::ui::theme::controls;
use eframe::egui;
use trellis_core::forge::Connection;
use trellis_core::remote_ops::{RemoteNameProblem, check_remote_name};
use trellis_core::repo::Snapshot;

fn spinner_line(ui: &mut egui::Ui, text: String) {
    pane_status::loading_line(ui, &text);
}

/// The remote name field and its problem line; returns (Enter pressed, the name is usable). The
/// field takes the focus when nothing has it, if `focus`.
fn name_field(ui: &mut egui::Ui, f: &mut ForkCreate, snap: Option<&Snapshot>, enabled: bool, focus: bool) -> (bool, bool) {
    let mut enter = false;
    ui.add_enabled_ui(enabled, |ui| {
        let (r, e) = field(ui, "Remote name", &mut f.name, "trellis-fork-create-name", "fork");
        form_message::track(&mut f.touched, "name", &r);
        if focus && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        enter = e;
    });
    let existing: Vec<&str> = snap.map(|s| s.remote_info.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
    let problem = check_remote_name(f.name.trim(), &existing, None).err();
    problem_line(
        ui,
        &f.touched,
        "name",
        problem.as_ref().filter(|p| **p != RemoteNameProblem::Empty).map(|p| p.message()).unwrap_or_default(),
    );
    (enter, problem.is_none())
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
    // (primary button label, enabled, message) when the step has one.
    let mut primary: Option<(&str, bool, RemoteMsg)> = None;
    let mut submit = false;
    match f.step.clone() {
        ForkStep::Checking => match &f.error {
            None => spinner_line(ui, format!("Looking for your fork of {repo}…")),
            Some(e) => {
                let error = error_text(e);
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
                }
                None => {
                    ui.add_space(6.0);
                    let (enter, ok) = name_field(ui, f, snap, true, focus);
                    ui.add_space(4.0);
                    controls::checkbox(ui, &mut f.fetch_after, "Fetch its branches after adding");
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
                ui.add_space(4.0);
                form_message::form(ui, Message::error(error_text(e)));
            }
            if creating {
                ui.add_space(4.0);
                spinner_line(ui, format!("Creating the fork on {service}… This can take a few seconds."));
            }
            submit = enter;
            primary = Some(("Fork and add remote", ok && !creating, RemoteMsg::CreateFork));
        }
    }
    let footer = match &primary {
        Some((label, enabled, _)) => Footer::new().cancel().primary(buttons::primary(*label).enabled(*enabled)).submit(submit),
        // While it looks for the fork, nothing has happened yet to close.
        None if matches!(f.step, ForkStep::Checking) => Footer::new().cancel(),
        None => Footer::new().close(),
    };
    let out = d.finish(ui, footer);
    if out.attempted {
        f.touched.attempt();
    }
    if out.primary
        && let Some((_, _, msg)) = primary
    {
        msgs.push(Msg::Remote(msg));
    }
    out.dismiss
}
