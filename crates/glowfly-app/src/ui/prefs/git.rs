//! Repo > Identity and Git hooks. The fields edit the loaded git config in place; the Save /
//! Revert bar writes them to the repository's config. The hooks page also turns git's built-in
//! file system monitor on or off for the repository, each a job of its own.

use super::{help, row, text_field};
use crate::msg::PrefsMsg;
use crate::msg::{FolderPurpose, LoadMsg, Msg};
use crate::state::prefs::GitConfigValues;
use crate::state::repo_identity::inherit_hints;
use crate::ui::widgets::buttons;
use eframe::egui::Ui;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::StatusSpeedups;
use glowfly_core::repo_identity::RepoIdentity;
use glowfly_core::settings::Settings;

/// What the hooks page's file system monitor row offers.
pub struct MonitorRow {
    /// Git's own daemon could be turned on here: this OS and git have it, and the repository
    /// isn't on a network drive.
    pub offered: bool,
    /// A job runs, so neither button can start another.
    pub busy: bool,
}

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

pub fn hooks_page(ui: &mut Ui, git: &mut GitConfigValues, monitor: &MonitorRow, t: Lang, msgs: &mut Vec<Msg>) {
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
    monitor_row(ui, git.fsmonitor, monitor, t, msgs);
}

/// Shown while the monitor is on (Turn off) or could be turned on (Turn on); with neither, as
/// on Linux or with a git built without it, the row is left out.
fn monitor_row(ui: &mut Ui, on: bool, m: &MonitorRow, t: Lang, msgs: &mut Vec<Msg>) {
    if !on && !m.offered {
        return;
    }
    ui.add_space(12.0);
    row(ui, t.tr("prefs.hooks.monitor.label"), |ui| {
        let why = m.busy.then(|| t.tr("common.busy"));
        let (text, msg) = if on {
            (t.tr("prefs.hooks.monitor.turn_off.button"), LoadMsg::TurnOffMonitor)
        } else {
            (
                t.tr("prefs.hooks.monitor.turn_on.button"),
                LoadMsg::SpeedUpStatus { speedups: StatusSpeedups::default(), cache: false, monitor: true },
            )
        };
        if ui.add(buttons::secondary(text).enabled(!m.busy).why(why)).clicked() {
            msgs.push(Msg::Load(msg));
        }
        help(ui, if on { t.tr("prefs.hooks.monitor.on.help") } else { t.tr("prefs.hooks.monitor.off.help") });
    });
}
