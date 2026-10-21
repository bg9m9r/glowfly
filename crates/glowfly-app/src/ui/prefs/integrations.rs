//! Preferences > Integrations (one page per hosting service: a card for each connected account,
//! then Add another account or the sign-in form, integrations_form.rs) and Repo > Issue
//! Tracker.

use super::integrations_form::{account_card, aligned, sign_in_form};
use super::{help, row, text_field, trello};
use crate::msg::{ForgeMsg, Msg, PrefsMsg};
use crate::state::profiles::accounts_owner;
use crate::state::{ForgeState, PrefsState};
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use eframe::egui::Ui;
use glowfly_core::forge::{Account, Provider};
use glowfly_core::settings::{Settings, TrackerChoice};
use std::path::Path;

pub fn integration_page(ui: &mut Ui, s: &mut Settings, prefs: &mut PrefsState, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let provider = prefs.int_provider;
    let t = s.lang();
    // Which profile the accounts switch with, over the cards.
    if let Some(p) = s.active_profile() {
        aligned(ui, |ui| help(ui, &accounts_owner(&p.name)));
        ui.add_space(6.0);
    }
    let mine: Vec<_> = s.connections.iter().filter(|c| c.account.provider == provider).cloned().collect();
    for c in &mine {
        aligned(ui, |ui| account_card(ui, c, msgs));
        ui.add_space(8.0);
    }
    if mine.is_empty() || prefs.int_adding {
        ui.add_space(6.0);
        sign_in_form(ui, t, prefs, !mine.is_empty(), msgs);
    } else {
        aligned(ui, |ui| {
            if ui.add(buttons::secondary("Add another account")).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::ShowAccountForm(true)));
            }
        });
    }
    ui.add_space(10.0);
    if provider.is_tracker_only() {
        let text = if provider == Provider::Trello {
            "Trello is an issue tracker. Once connected, choose it and a board for a repository under Repo-specific preferences > Issue tracker."
        } else {
            "Jira is an issue tracker. Once connected, choose it for a repository under Repo-specific preferences > Issue tracker."
        };
        row(ui, "", |ui| help(ui, text));
    }
    if let Some(var) = provider.env_var() {
        row(ui, "", |ui| {
            help(ui, &format!("If the {var} environment variable is set, Trellis uses it ahead of connections made here."));
        });
    }
    if let Some(t) = fs.and_then(|f| f.host.as_ref()).filter(|t| t.provider == provider) {
        row(ui, "", |ui| {
            help(ui, &format!("The open repository is on {} ({}).", provider.name(), t.display()));
        });
    }
}

pub fn tracker_page(
    ui: &mut Ui,
    s: &mut Settings,
    prefs: &mut PrefsState,
    fs: Option<&ForgeState>,
    workdir: Option<&Path>,
    msgs: &mut Vec<Msg>,
) {
    let Some(workdir) = workdir else { return };
    let current = s.tracker_for(workdir);
    let host = fs.and_then(|f| f.host.as_ref());
    let trackers: Vec<Account> = s.connections.iter().filter(|c| c.account.provider.is_tracker_only()).map(|c| c.account.clone()).collect();
    row(ui, "Issues come from", |ui| {
        let auto_label = match host {
            Some(t) if t.provider == Provider::AzureDevOps => "Azure Boards (from the remote)".to_owned(),
            Some(t) if t.provider.has_issues() => format!("{} issues (from the remote)", t.provider.name()),
            Some(t) => format!("Automatic ({} has no issue tracker)", t.provider.name()),
            None => "Automatic (remote not recognized)".to_owned(),
        };
        if controls::radio(ui, current == TrackerChoice::Auto, auto_label).clicked() {
            msgs.push(Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Auto)));
        }
        for a in &trackers {
            let selected = matches!(&current, TrackerChoice::Jira { account, .. } | TrackerChoice::Trello { account, .. } if account == a);
            // Clicking the chosen one again keeps its project or board.
            if controls::radio(ui, selected, format!("{} · {}", a.provider.name(), a.host)).clicked() && !selected {
                let choice = if a.provider == Provider::Trello {
                    TrackerChoice::Trello { account: a.clone(), board: String::new(), board_name: String::new() }
                } else {
                    TrackerChoice::Jira { account: a.clone(), project: prefs.jira_project.trim().to_owned() }
                };
                msgs.push(Msg::Forge(ForgeMsg::SetTracker(choice)));
            }
        }
        if controls::radio(ui, current == TrackerChoice::None, "None").clicked() {
            msgs.push(Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::None)));
        }
        if host.is_some_and(|h| h.provider == Provider::BitbucketDataCenter) {
            help(ui, "Bitbucket Data Center keeps no issues of its own. Connect Jira under Integrations and choose it here.");
        }
        if trackers.is_empty() {
            help(ui, "Connect Jira Cloud, Jira Data Center or Trello under Integrations to use it here.");
        }
    });
    if let TrackerChoice::Trello { account, board, board_name } = &current {
        trello::board_row(ui, account, board, board_name, &prefs.trello_boards, msgs);
    }
    if let TrackerChoice::Jira { account, project } = &current {
        row(ui, "Jira project", |ui| {
            let r = text_field(ui, &mut prefs.jira_project, "all projects");
            if r.lost_focus() && prefs.jira_project.trim() != project {
                msgs.push(Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Jira {
                    account: account.clone(),
                    project: prefs.jira_project.trim().to_uppercase(),
                })));
            }
            help(
                ui,
                "Optional project key (e.g. PROJ) to limit the ISSUES list. Mentions like PROJ-12 in commits and branch names link to Jira either way.",
            );
        });
    }
}
