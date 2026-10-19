//! Repo > Identity and Git hooks. The fields edit the loaded git config in place; the Save /
//! Revert bar writes them to the repository's config.

use super::{help, row, text_field};
use crate::msg::PrefsMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::prefs::GitConfigValues;
use crate::ui::widgets::buttons;
use eframe::egui::Ui;

pub fn identity_page(ui: &mut Ui, git: &mut GitConfigValues) {
    let global_name = git.user_name.clone();
    let global_email = git.user_email.clone();
    row(ui, "Name", |ui| {
        let hint = if global_name.is_empty() { "not set".to_owned() } else { format!("inherits {global_name}") };
        text_field(ui, &mut git.repo_name, &hint);
    });
    row(ui, "Email", |ui| {
        let hint = if global_email.is_empty() { "not set".to_owned() } else { format!("inherits {global_email}") };
        text_field(ui, &mut git.repo_email, &hint);
        help(ui, "Overrides your profile for this repository only (local Git config). Leave empty to use your profile.");
    });
}

pub fn hooks_page(ui: &mut Ui, git: &mut GitConfigValues, msgs: &mut Vec<Msg>) {
    row(ui, "Hooks path", |ui| {
        ui.horizontal(|ui| {
            text_field(ui, &mut git.hooks_path, ".git/hooks");
            // Only fills the field.
            if ui.add(buttons::secondary("Browse")).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::HooksPath)));
            }
        });
        help(
            ui,
            "Sets core.hooksPath for this repository. Trellis commits, pushes and merges through git, so hooks in this folder run automatically. Leave empty to use .git/hooks.",
        );
    });
}
