//! Preferences > SSH: Key for SSH Remotes (what core.sshCommand says globally and in the open
//! repository, with Stop using) and SSH Agent (the Use local SSH agent setting, what the agent
//! holds, and why it may not be reachable). The view never edits settings; it sends `Msg`s.

use super::ssh::SshContext;
use super::{help, row, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use crate::state::ssh_use::{
    GIT_SSH_COMMAND_SET, LOOKING, NO_AUTH_SOCK, NOT_SET, OTHER_WITH_AGENT, PAGEANT_NOTE, USE_AGENT_HELP, WINDOWS_AGENT_NOTE, agent_summary,
    service_line,
};
use crate::state::trust::shown_value;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, RichText, Ui};
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{AgentReach, KeyChoice, SshListing, WinService};

pub(super) fn choice_row(ui: &mut Ui, page: &SshPage, listing: &SshListing, cx: &SshContext, msgs: &mut Vec<Msg>) {
    row(ui, "Key for SSH Remotes", |ui| {
        let Some(choices) = &page.choice else {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0));
                ui.label(RichText::new("Reading core.sshCommand…").size(12.0).color(color::weak()));
            });
            return;
        };
        level(ui, "All repositories", &choices.global, false, listing, cx, msgs);
        if let (Some(repo), Some(choice)) = (&page.repo, &choices.repo) {
            ui.add_space(4.0);
            level(ui, &format!("{} only", repo.name), choice, true, listing, cx, msgs);
            if matches!(choice, Ok(KeyChoice::Key(_) | KeyChoice::Other { .. })) {
                help(ui, &format!("The {} setting overrides the one for all repositories in this repository.", repo.name));
            }
        }
        if page.agent.as_ref().is_some_and(|a| a.git_ssh_command_set) {
            warn(ui, GIT_SSH_COMMAND_SET);
        }
        if page.choice_busy {
            ui.add(egui::Spinner::new().size(14.0));
        }
        if let Some(e) = &page.choice_error {
            warn(ui, e);
        }
    });
}

/// One level's line: its name, what it says, and Stop using when something is set.
fn level(
    ui: &mut Ui,
    name: &str,
    choice: &Result<KeyChoice, String>,
    repo: bool,
    listing: &SshListing,
    cx: &SshContext,
    msgs: &mut Vec<Msg>,
) {
    ui.label(RichText::new(name).size(12.5).strong().color(color::text()));
    let stop = |ui: &mut Ui, msgs: &mut Vec<Msg>| {
        if ui.add(buttons::secondary("Stop using").small()).clicked() {
            msgs.push(Msg::Ssh(SshMsg::StopUsing { repo, confirmed: false }));
        }
    };
    match choice {
        Err(e) => warn(ui, e),
        Ok(KeyChoice::Unset) => help(ui, NOT_SET),
        Ok(KeyChoice::Key(path)) => {
            let shown = listing.keys.iter().find(|k| same_spelling(&k.private, path)).map(|k| k.file_name());
            ui.horizontal(|ui| {
                ui.label(RichText::new(shown.unwrap_or_else(|| path.display().to_string())).monospace().size(12.0).color(color::text()));
                stop(ui, msgs);
            });
        }
        Ok(KeyChoice::Other { value, .. }) => {
            warn(ui, &format!("Set outside Trellis: {}", shown_value(value)));
            if cx.use_agent {
                help(ui, OTHER_WITH_AGENT);
            }
            stop(ui, msgs);
        }
    }
}

pub(super) fn agent_row(ui: &mut Ui, page: &SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    row(ui, "SSH Agent", |ui| {
        let mut on = cx.use_agent;
        if ui.add(controls::Check::new(&mut on, "Use local SSH agent")).changed() {
            msgs.push(Msg::Ssh(SshMsg::SetUseAgent(on)));
        }
        help(ui, USE_AGENT_HELP);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            match &page.agent {
                None => {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label(RichText::new(LOOKING).size(12.0).color(color::weak()));
                }
                Some(a) => {
                    ui.label(RichText::new(agent_summary(a)).size(12.5).color(color::text()));
                }
            }
            if widgets::icon_button(ui, Icon::Refresh, "Check the agent again").clicked() {
                msgs.push(Msg::Ssh(SshMsg::ReloadAgent));
            }
        });
        let Some(a) = &page.agent else { return };
        if let Some(AgentReach::Keys(keys)) = &a.reach {
            for k in keys {
                let line = if k.comment.is_empty() { k.fingerprint.clone() } else { format!("{}  {}", k.fingerprint, k.comment) };
                ui.label(RichText::new(line).monospace().size(11.5).color(color::faint()));
            }
        }
        // The service is only looked up on Windows; without it, this is another system.
        match a.service {
            Some(s) => {
                help(ui, service_line(&s));
                if s == WinService::Running && a.running() && a.git_ssh_env.is_none() {
                    help(ui, WINDOWS_AGENT_NOTE);
                }
            }
            None if !a.auth_sock_set && !a.running() => warn(ui, NO_AUTH_SOCK),
            None => {}
        }
        if a.pageant {
            help(ui, PAGEANT_NOTE);
        }
        if let Some(e) = &page.agent_error {
            warn(ui, e);
        }
    });
}
