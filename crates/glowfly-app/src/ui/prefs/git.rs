//! Repo > Identity and Git hooks. The fields edit the loaded git config in place; the Save /
//! Revert bar writes them to the repository's config.

use super::{help, row, text_field};
use crate::msg::PrefsMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::prefs::GitConfigValues;
use crate::state::repo_identity::inherit_hints;
use crate::ui::widgets::buttons;
use eframe::egui::Ui;
use glowfly_core::repo_identity::RepoIdentity;
use glowfly_core::settings::Settings;

/// `inherited`: the repository's identity as git last read it, so the hints name a profile's
/// folder rule when one applies.
pub fn identity_page(ui: &mut Ui, git: &mut GitConfigValues, inherited: Option<&RepoIdentity>, settings: &Settings) {
    let (name_hint, email_hint) = inherit_hints(inherited, settings, &git.user_name, &git.user_email);
    row(ui, "Name", |ui| {
        text_field(ui, &mut git.repo_name, &name_hint);
    });
    row(ui, "Email", |ui| {
        text_field(ui, &mut git.repo_email, &email_hint);
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
            "Sets core.hooksPath for this repository. Glowfly commits, pushes and merges through git, so hooks in this folder run automatically. Leave empty to use .git/hooks.",
        );
    });
}
