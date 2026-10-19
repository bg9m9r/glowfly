//! Preferences > Integrations (one page per hosting service) and Repo > Issue Tracker.

use super::{LABEL_W, help, primary_button, row, text_field, trello};
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::profiles::accounts_owner;
use crate::state::{ForgeState, PrefsState};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use crate::ui::widgets::form_message::{self, Message};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, RichText, Sense, Stroke, Ui, vec2};
use std::path::Path;
use trellis_core::forge::{self, Account, Provider, Target};
use trellis_core::settings::{Settings, TrackerChoice};

pub fn integration_page(ui: &mut Ui, s: &mut Settings, prefs: &mut PrefsState, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let provider = prefs.int_provider;
    let account = forge::parse_host(provider, &prefs.int_host).map(|(host, org)| Account { provider, host, org });
    let connection = account.as_ref().and_then(|a| s.connection(a)).cloned();

    // Which profile the accounts switch with, over the card.
    if let Some(p) = s.active_profile() {
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W - 20.0);
            help(ui, &accounts_owner(&p.name));
        });
        ui.add_space(6.0);
    }
    // Connection card, centered over the form.
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W - 20.0);
        egui::Frame::new().fill(color::toolbar()).corner_radius(4).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(26.0, 40.0), Sense::hover());
                ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(1.5_f32, color::weak()));
                widgets::paint_icon(ui.painter(), r.center(), Icon::User, color::weak());
                ui.vertical(|ui| match &connection {
                    Some(c) => {
                        ui.label(RichText::new("Connected").color(color::green()));
                        ui.label(
                            RichText::new(format!("{} · {} via {}", c.account.display(), c.user, c.method))
                                .size(type_scale::SMALL)
                                .color(color::weak()),
                        );
                        if ui.add(buttons::secondary("Disconnect").small()).clicked() {
                            msgs.push(Msg::Forge(ForgeMsg::Disconnect(c.account.clone())));
                        }
                    }
                    None => {
                        ui.label(RichText::new("Not connected").color(color::red()));
                        if let Some(a) = &account {
                            ui.label(RichText::new(a.display()).size(type_scale::SMALL).color(color::weak()));
                        }
                        if provider.supports_gcm() {
                            ui.horizontal(|ui| {
                                let enabled = !prefs.int_busy && account.is_some();
                                let label = format!("Connect to {}", provider.name());
                                let tip = if account.is_some() {
                                    "Sign in with Git Credential Manager (the same sign-in git push uses)"
                                } else {
                                    "Enter the host domain first"
                                };
                                if primary_button(ui, &label, enabled).on_hover_text(tip).clicked()
                                    && let Some(a) = &account
                                {
                                    msgs.push(Msg::Forge(ForgeMsg::Connect { account: a.clone(), user: None, token: None }));
                                }
                                if prefs.int_busy {
                                    ui.add(egui::Spinner::new().size(14.0));
                                }
                            });
                        }
                    }
                });
            });
        });
    });
    ui.add_space(14.0);
    if provider.supports_gcm() {
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W + 60.0);
            ui.label(RichText::new("or").size(type_scale::BODY).color(color::text()));
        });
        ui.add_space(14.0);
    }

    if provider.fixed_host().is_none() {
        row(ui, "Host domain", |ui| {
            text_field(ui, &mut prefs.int_host, provider.host_hint());
        });
    }
    let is_trello = provider == Provider::Trello;
    if let Some(label) = provider.username_label() {
        row(ui, label, |ui| {
            let hint = match label {
                "Email" => "you@example.com",
                _ if is_trello => "32-character key",
                _ => "username",
            };
            text_field(ui, &mut prefs.int_user, hint);
            if is_trello {
                trello::key_link(ui, msgs);
            }
        });
    }
    let token_label = match provider {
        Provider::Bitbucket => "App password",
        Provider::JiraCloud => "API token",
        Provider::Trello => "Token",
        _ => "Personal access token",
    };
    row(ui, token_label, |ui| {
        controls::field(ui, egui::TextEdit::singleline(&mut prefs.int_token).password(true).desired_width(320.0).margin(vec2(8.0, 5.0)));
        help(ui, &format!("{} Stored in the Windows Credential Manager, never in Trellis's settings file.", provider.token_help()));
        if is_trello {
            trello::token_link(ui, &prefs.int_user, msgs);
        } else if let Some(url) = account.as_ref().and_then(Target::token_url)
            && ui.link("Create a token").clicked()
        {
            msgs.push(Msg::OpenUrl(url));
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 18.0);
        let needs_user = matches!(provider, Provider::JiraCloud | Provider::Trello) && prefs.int_user.trim().is_empty();
        let valid = !prefs.int_token.trim().is_empty() && account.is_some() && !prefs.int_busy && !needs_user;
        if primary_button(ui, "Connect", valid).clicked()
            && let Some(a) = &account
        {
            let user = Some(prefs.int_user.trim().to_owned()).filter(|u| !u.is_empty());
            msgs.push(Msg::Forge(ForgeMsg::Connect { account: a.clone(), user, token: Some(prefs.int_token.trim().to_owned()) }));
        }
        if prefs.int_busy {
            ui.add(egui::Spinner::new().size(14.0));
        }
    });
    if let Some(e) = &prefs.int_error {
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W + 18.0);
            ui.vertical(|ui| form_message::line(ui, Message::error(e.as_str())));
        });
    }
    if provider.is_tracker_only() {
        ui.add_space(10.0);
        let text = if is_trello {
            "Trello is an issue tracker. Once connected, choose it and a board for a repository under Repo-specific preferences > Issue tracker."
        } else {
            "Jira is an issue tracker. Once connected, choose it for a repository under Repo-specific preferences > Issue tracker."
        };
        row(ui, "", |ui| help(ui, text));
    }

    let mine: Vec<_> = s.connections.iter().filter(|c| c.account.provider == provider).cloned().collect();
    if !mine.is_empty() {
        ui.add_space(20.0);
        row(ui, "Connected accounts", |ui| {
            for c in mine {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(c.account.display()).color(color::text()));
                    ui.label(RichText::new(format!("{} · {}", c.user, c.method)).size(type_scale::SMALL).color(color::weak()));
                    if ui.add(buttons::secondary("Disconnect").small()).clicked() {
                        msgs.push(Msg::Forge(ForgeMsg::Disconnect(c.account.clone())));
                    }
                });
            }
        });
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
