//! Preferences > General.

use super::{help, row, text_field, warn};
use crate::msg::Msg;
use crate::state::files::TRASH;
use crate::state::prefs::GitConfigValues;
use crate::ui::theme::color;
use eframe::egui::{self, Color32, RichText, Ui};
use trellis_core::settings::Settings;

pub fn general_page(ui: &mut Ui, s: &mut Settings, git: Option<&mut GitConfigValues>, msgs: &mut Vec<Msg>) {
    row(ui, "Auto-Fetch Interval", |ui| {
        ui.add(egui::DragValue::new(&mut s.auto_fetch_minutes).range(0..=60).suffix(" min"));
        help(ui, "In minutes, between 0 and 60 (0 disables auto-fetch). Fetches all remotes of the open repository.");
        warn(ui, "If you have many remotes, this may impact performance.");
    });
    row(ui, "Auto-Prune", |ui| {
        ui.checkbox(&mut s.auto_prune, "Remove remote-tracking branches that no longer exist on the remote when fetching");
    });
    row(ui, "Default Branch Name", |ui| {
        text_field(ui, &mut s.default_branch, "main");
        help(ui, "Sets the default branch name when initializing new repositories.");
    });
    row(ui, "Initial Commits in Graph", |ui| {
        ui.add(egui::DragValue::new(&mut s.initial_commits).range(trellis_core::settings::MIN_INITIAL_COMMITS..=200_000).speed(50));
        help(ui, "Minimum 500. Takes effect when you leave Preferences or refresh.");
        warn(ui, "Increasing this value may adversely affect performance.");
    });
    row(ui, "Reopen Last Repository", |ui| {
        ui.checkbox(&mut s.reopen_last_repo, "Reopen the tabs from last time when Trellis starts");
    });
    row(ui, "Watch for File Changes", |ui| {
        ui.checkbox(&mut s.watch_files, "Refresh the working tree as soon as files change outside Trellis");
        help(ui, "When off, Trellis refreshes when its window regains focus or you press F5.");
    });
    row(ui, "Deleting Files", |ui| {
        ui.checkbox(&mut s.delete_to_trash, format!("Move deleted files to the {TRASH}"));
        help(ui, "When off, Delete… and discarding an untracked file remove it from disk straight away, and it can't be recovered.");
    });
    if let Some(git) = git {
        row(ui, "Longpaths", |ui| {
            if ui.checkbox(&mut git.longpaths, "").changed() {
                msgs.push(Msg::GitConfig { global: true, key: "core.longpaths".into(), value: Some(git.longpaths.to_string()) });
            }
            help(
                ui,
                "Sets core.longpaths in your global Git config. By default, Windows only supports file paths less than 260 characters. Enable support for longer file paths.",
            );
        });
        row(ui, "AutoCRLF", |ui| {
            if ui.checkbox(&mut git.autocrlf, "").changed() {
                msgs.push(Msg::GitConfig { global: true, key: "core.autocrlf".into(), value: Some(git.autocrlf.to_string()) });
            }
            help(ui, "Sets core.autocrlf in your global Git config.");
        });
    }
    row(ui, "Forget All Connections", |ui| {
        let n = s.connections.len();
        let danger = |text: &str, on: bool| {
            egui::Button::new(RichText::new(text).color(if on { Color32::WHITE } else { color::faint() })).fill(if on {
                color::danger()
            } else {
                color::danger_off()
            })
        };
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, danger("Forget All", n > 0)).clicked() {
                msgs.push(Msg::ForgetTokens { gcm: false });
            }
            let gcm_hosts = s.connections.iter().filter(|c| c.account.provider.supports_gcm()).count();
            if ui.add_enabled(gcm_hosts > 0, danger("Forget All + Git Credential Manager", gcm_hosts > 0)).clicked() {
                msgs.push(Msg::confirm(
                    "Sign out of Git Credential Manager",
                    format!(
                        "This disconnects every integration and also removes the sign-ins Git Credential Manager stored for {gcm_hosts} host{}. \
                         git push and pull to those hosts will ask you to sign in again.",
                        if gcm_hosts == 1 { "" } else { "s" }
                    ),
                    "Forget All",
                    Msg::ForgetTokens { gcm: true },
                ));
            }
        });
        if n == 0 {
            help(ui, "Trellis has no stored connections to remove.");
        } else {
            help(
                ui,
                &format!(
                    "Disconnects {n} integration{} and deletes their saved tokens. The second button also signs Git Credential Manager out of those hosts.",
                    if n == 1 { "" } else { "s" }
                ),
            );
        }
    });
}
