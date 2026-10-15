//! Preferences > Integrations (one page per hosting service) and Repo > Issue Tracker.

use super::{LABEL_W, help, primary_button, row, text_field};
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::{ForgeState, PrefsState};
use crate::ui::theme::color;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, RichText, Sense, Stroke, Ui, vec2};
use std::path::Path;
use trellis_core::forge::{self, Account, Provider, Target};
use trellis_core::settings::{Settings, TrackerChoice};

pub fn integration_page(ui: &mut Ui, s: &mut Settings, prefs: &mut PrefsState, fs: Option<&ForgeState>, msgs: &mut Vec<Msg>) {
    let provider = prefs.int_provider;
    let account = forge::parse_host(provider, &prefs.int_host).map(|(host, org)| Account { provider, host, org });
    let connection = account.as_ref().and_then(|a| s.connection(a)).cloned();

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
                            RichText::new(format!("{} · {} via {}", c.account.display(), c.user, c.method)).size(12.0).color(color::weak()),
                        );
                        if ui.small_button("Disconnect").clicked() {
                            msgs.push(Msg::Forge(ForgeMsg::Disconnect(c.account.clone())));
                        }
                    }
                    None => {
                        ui.label(RichText::new("Not Connected").color(color::red()));
                        if let Some(a) = &account {
                            ui.label(RichText::new(a.display()).size(12.0).color(color::weak()));
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
            ui.label(RichText::new("or").size(14.0).color(color::text()));
        });
        ui.add_space(14.0);
    }

    if provider.fixed_host().is_none() {
        row(ui, "Host Domain", |ui| {
            text_field(ui, &mut prefs.int_host, provider.host_hint());
        });
    }
    if let Some(label) = provider.username_label() {
        row(ui, label, |ui| {
            text_field(ui, &mut prefs.int_user, if label == "Email" { "you@example.com" } else { "username" });
        });
    }
    let token_label = if provider == Provider::Bitbucket {
        "App Password"
    } else if provider.is_jira() && provider == Provider::JiraCloud {
        "API Token"
    } else {
        "Personal Access Token"
    };
    row(ui, token_label, |ui| {
        ui.add(egui::TextEdit::singleline(&mut prefs.int_token).password(true).desired_width(320.0).margin(vec2(8.0, 5.0)));
        help(ui, &format!("{} Stored in the Windows Credential Manager, never in Trellis's settings file.", provider.token_help()));
        if let Some(url) = account.as_ref().and_then(Target::token_url)
            && ui.link("Create a token").clicked()
        {
            msgs.push(Msg::OpenUrl(url));
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 18.0);
        let needs_user = provider == Provider::JiraCloud && prefs.int_user.trim().is_empty();
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
            ui.label(RichText::new(e).color(color::red()));
        });
    }
    if provider.is_jira() {
        ui.add_space(10.0);
        row(ui, "", |ui| {
            help(
                ui,
                "Jira is an issue tracker. Once connected, choose it for a repository under Repo-Specific Preferences > Issue Tracker.",
            );
        });
    }

    let mine: Vec<_> = s.connections.iter().filter(|c| c.account.provider == provider).cloned().collect();
    if !mine.is_empty() {
        ui.add_space(20.0);
        row(ui, "Connected Accounts", |ui| {
            for c in mine {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(c.account.display()).color(color::text()));
                    ui.label(RichText::new(format!("{} · {}", c.user, c.method)).size(12.0).color(color::weak()));
                    if ui.small_button("Disconnect").clicked() {
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
    let jira: Vec<Account> = s.connections.iter().filter(|c| c.account.provider.is_jira()).map(|c| c.account.clone()).collect();
    row(ui, "Issues Come From", |ui| {
        let auto_label = match host {
            Some(t) if t.provider == Provider::AzureDevOps => "Azure Boards (from the remote)".to_owned(),
            Some(t) if t.provider.has_issues() => format!("{} issues (from the remote)", t.provider.name()),
            Some(t) => format!("Automatic ({} has no issue tracker)", t.provider.name()),
            None => "Automatic (remote not recognized)".to_owned(),
        };
        if ui.radio(current == TrackerChoice::Auto, auto_label).clicked() {
            msgs.push(Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Auto)));
        }
        for a in &jira {
            let selected = matches!(&current, TrackerChoice::Jira { account, .. } if account == a);
            if ui.radio(selected, format!("{} · {}", a.provider.name(), a.host)).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Jira {
                    account: a.clone(),
                    project: prefs.jira_project.trim().to_owned(),
                })));
            }
        }
        if ui.radio(current == TrackerChoice::None, "None").clicked() {
            msgs.push(Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::None)));
        }
        if jira.is_empty() {
            help(ui, "Connect Jira Cloud or Jira Data Center under Integrations to use it here.");
        }
    });
    if let TrackerChoice::Jira { account, project } = &current {
        row(ui, "Jira Project", |ui| {
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
