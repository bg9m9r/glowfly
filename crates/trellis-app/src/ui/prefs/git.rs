//! Repo > Identity and Git Hooks.

use super::{help, row, text_field};
use crate::msg::PrefsMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::prefs::GitConfigValues;
use eframe::egui::Ui;

pub fn identity_page(ui: &mut Ui, git: &mut GitConfigValues, msgs: &mut Vec<Msg>) {
    let global_name = git.user_name.clone();
    let global_email = git.user_email.clone();
    row(ui, "Name", |ui| {
        let hint = if global_name.is_empty() { "not set".to_owned() } else { format!("inherits {global_name}") };
        if text_field(ui, &mut git.repo_name, &hint).lost_focus() {
            msgs.push(Msg::Prefs(PrefsMsg::GitConfig {
                global: false,
                key: "user.name".into(),
                value: Some(git.repo_name.trim().to_owned()).filter(|v| !v.is_empty()),
            }));
        }
    });
    row(ui, "Email", |ui| {
        let hint = if global_email.is_empty() { "not set".to_owned() } else { format!("inherits {global_email}") };
        if text_field(ui, &mut git.repo_email, &hint).lost_focus() {
            msgs.push(Msg::Prefs(PrefsMsg::GitConfig {
                global: false,
                key: "user.email".into(),
                value: Some(git.repo_email.trim().to_owned()).filter(|v| !v.is_empty()),
            }));
        }
        help(ui, "Overrides your profile for this repository only (local Git config). Leave empty to use your profile.");
    });
}

pub fn hooks_page(ui: &mut Ui, git: &mut GitConfigValues, msgs: &mut Vec<Msg>) {
    row(ui, "Hooks Path", |ui| {
        let mut commit = false;
        ui.horizontal(|ui| {
            commit = text_field(ui, &mut git.hooks_path, ".git/hooks").lost_focus();
            if ui.button("Browse").clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::HooksPath)));
            }
        });
        if commit {
            msgs.push(Msg::Prefs(PrefsMsg::GitConfig {
                global: false,
                key: "core.hooksPath".into(),
                value: Some(git.hooks_path.trim().to_owned()).filter(|v| !v.is_empty()),
            }));
        }
        help(
            ui,
            "Sets core.hooksPath for this repository. Trellis commits, pushes and merges through git, so hooks in this folder run automatically. Leave empty to use .git/hooks.",
        );
    });
}
