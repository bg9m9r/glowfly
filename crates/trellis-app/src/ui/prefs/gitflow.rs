//! Preferences > Git Flow (repository-specific): whether Git Flow is set up, the production and
//! development branches with a note on how each would be created, the branch and tag prefixes,
//! Initialize / Save, buttons that start a feature, release or hotfix, and a row that picks an
//! open one to finish. Every action is a `Msg`; the checks are the pure ones from
//! `trellis_core::gitflow`.

use super::appearance::section;
use super::{help, primary_button, row, text_field, warn};
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::blocked::BUSY;
use crate::state::gitflow::GitFlowPage;
use crate::ui::theme::color;
use crate::ui::widgets::buttons;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText, Ui};
use trellis_core::branch_ops::is_valid_branch_name;
use trellis_core::gitflow::{BranchPlan, Branches, FlowKind, GitFlowSettings, InitProblem, check_settings, flow_branches, plan_branches};
use trellis_core::repo::Snapshot;

/// What the page needs from outside Preferences.
pub struct GitFlowContext<'a> {
    pub snap: Option<&'a Snapshot>,
    /// A git job is running.
    pub busy: bool,
}

/// A line under a branch field.
enum Note {
    Help(String),
    Warn(String),
}

pub fn gitflow_page(ui: &mut Ui, page: &mut GitFlowPage, cx: &GitFlowContext, msgs: &mut Vec<Msg>) {
    let Some(snap) = cx.snap else {
        help(ui, "Open a repository to set up Git Flow.");
        return;
    };
    let b = Branches::of(snap);
    let saved = snap.gitflow.settings();
    status_row(ui, saved.is_some(), &snap.name);

    section(ui, "Branches");
    let initialized = saved.is_some();
    let mut changed = branch_row(ui, "Production branch", &mut page.form.production, "main", |p| production_note(p, &b));
    let production = page.form.production.trim().to_owned();
    changed |=
        branch_row(ui, "Development branch", &mut page.form.development, "develop", |d| development_note(d, &production, initialized, &b));

    section(ui, "Prefixes");
    changed |= prefix_rows(ui, &mut page.form);
    if changed {
        page.error = None;
    }

    buttons(ui, page, &b, initialized, cx.busy, msgs);
    if let Some(s) = &saved {
        start_row(ui, s, &b, cx.busy, msgs);
        finish_row(ui, &mut page.finish_branch, s, &b, cx.busy, msgs);
    }
}

fn status_row(ui: &mut Ui, initialized: bool, repo: &str) {
    row(ui, "Status", |ui| {
        let text = if initialized { format!("Set up in {repo}") } else { format!("Not set up in {repo}") };
        ui.label(RichText::new(text).color(color::text()));
        help(
            ui,
            "Finished work lives on a production branch and work is combined on a development branch. New work happens on \
             feature, release and hotfix branches, each named with its own prefix. These settings are kept in this \
             repository's git config.",
        );
    });
}

/// A branch field with its note; true when the text changed.
fn branch_row(ui: &mut Ui, label: &str, value: &mut String, hint: &str, note: impl FnOnce(&str) -> Option<Note>) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        changed = text_field(ui, value, hint).changed();
        match note(value.trim()) {
            Some(Note::Help(t)) => help(ui, &t),
            Some(Note::Warn(t)) => warn(ui, &t),
            None => {}
        }
    });
    changed
}

fn production_note(name: &str, b: &Branches) -> Option<Note> {
    if !b.has_commits {
        return Some(Note::Warn("Make a first commit before setting up Git Flow.".into()));
    }
    if name.is_empty() || b.local(name) {
        return None;
    }
    Some(match b.on_remote(name) {
        Some(r) => Note::Help(format!("Created from {r}, tracking it.")),
        None => Note::Warn(format!("{name} doesn't exist.")),
    })
}

fn development_note(name: &str, production: &str, initialized: bool, b: &Branches) -> Option<Note> {
    if !b.has_commits || name.is_empty() || b.local(name) || !is_valid_branch_name(name) {
        return None;
    }
    Some(Note::Help(match b.on_remote(name) {
        Some(r) => format!("Created from {r}, tracking it."),
        None if initialized => format!("Created from {production} when you save."),
        None => format!("Created from {production} when you initialize."),
    }))
}

/// The three branch prefixes and the version tag prefix; true when one changed.
fn prefix_rows(ui: &mut Ui, form: &mut GitFlowSettings) -> bool {
    let mut changed = false;
    for (label, value, hint) in
        [("Feature", &mut form.feature, "feature/"), ("Release", &mut form.release, "release/"), ("Hotfix", &mut form.hotfix, "hotfix/")]
    {
        row(ui, label, |ui| changed |= text_field(ui, value, hint).changed());
    }
    row(ui, "Version tag", |ui| {
        changed |= text_field(ui, &mut form.version_tag, "none").changed();
        help(ui, "Put in front of the version in release and hotfix tag names, for example v.");
    });
    changed
}

/// The prefix warning, the first problem, Initialize / Save and Discard changes.
fn buttons(ui: &mut Ui, page: &GitFlowPage, b: &Branches, initialized: bool, busy: bool, msgs: &mut Vec<Msg>) {
    let form = page.form.trimmed();
    let checked = check_settings(&form);
    let plan = plan_branches(&form, b);
    // A missing production branch or no commits already show under the field.
    let problem = match (&checked, &plan) {
        (Err(p), _) => Some(p.message()),
        (Ok(()), Err(p @ InitProblem::DevelopmentName(..))) => Some(p.message()),
        _ => page.error.clone(),
    };
    let missing = matches!(&plan, Ok((p, d)) if *p != BranchPlan::Exists || *d != BranchPlan::Exists);
    let edited = page.edited();
    let enabled = !busy && checked.is_ok() && plan.is_ok() && (!initialized || edited || missing);
    row(ui, "", |ui| {
        if page.prefixes_changed() {
            warn(ui, "Branches named with the old prefixes will no longer count as Git Flow branches.");
        }
        if let Some(p) = &problem {
            form_message::line(ui, Message::error(p.as_str()));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let label = if initialized { "Save" } else { "Initialize Git Flow" };
            // A problem line above already says why; else busy, or nothing to save.
            let why = if problem.is_some() {
                None
            } else if busy {
                Some(BUSY)
            } else {
                (initialized && !edited && !missing).then_some("Nothing changed")
            };
            if primary_button(ui, label, enabled, why).clicked() {
                msgs.push(Msg::GitFlow(GitFlowMsg::Apply));
            }
            if edited && ui.add(buttons::secondary("Discard changes")).clicked() {
                msgs.push(Msg::GitFlow(GitFlowMsg::Discard));
            }
        });
    });
}

/// Start feature / release / hotfix, each from its saved base branch.
fn start_row(ui: &mut Ui, s: &GitFlowSettings, b: &Branches, busy: bool, msgs: &mut Vec<Msg>) {
    row(ui, "Start", |ui| {
        ui.horizontal_wrapped(|ui| {
            for kind in FlowKind::ALL {
                let base = s.base(kind);
                let exists = b.local(base);
                let r = ui.add(buttons::secondary(format!("Start {}…", kind.noun())).enabled(!busy && exists));
                let hover = if exists {
                    format!("Branch from {base}")
                } else if kind == FlowKind::Hotfix {
                    format!("{base} is needed to start a hotfix, and it doesn't exist.")
                } else {
                    format!("{base} doesn't exist. Save to create it.")
                };
                if r.on_hover_text(&hover).clicked() {
                    msgs.push(Msg::GitFlow(GitFlowMsg::StartDialog(kind)));
                }
            }
        });
    });
}

/// Pick an open feature, release or hotfix branch (the checked-out one by default) and open the
/// Finish dialog for it.
fn finish_row(ui: &mut Ui, choice: &mut String, s: &GitFlowSettings, b: &Branches, busy: bool, msgs: &mut Vec<Msg>) {
    let open = flow_branches(s, b);
    row(ui, "Finish", |ui| {
        let Some((_, first)) = open.first() else {
            help(ui, "No feature, release or hotfix branch is open.");
            return;
        };
        if !open.iter().any(|(_, n)| n == choice) {
            *choice = (*first).to_owned();
        }
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("gitflow-finish-branch").selected_text(choice.as_str()).width(220.0).show_ui(ui, |ui| {
                for (_, n) in &open {
                    ui.selectable_value(choice, (*n).to_owned(), *n);
                }
            });
            if ui.add(buttons::secondary("Finish…").enabled(!busy)).clicked() {
                msgs.push(Msg::GitFlow(GitFlowMsg::FinishDialog(choice.clone())));
            }
        });
    });
}
