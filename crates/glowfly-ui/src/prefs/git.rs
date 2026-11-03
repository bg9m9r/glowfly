//! Repo > Commit identity (opening with whom the repository commits as) and Git hooks. The fields edit the loaded git config in place; the Save /
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
    let t = settings.lang();
    super::intro(ui, &commit_as(t, inherited, settings, git));
    let (name_hint, email_hint) = inherit_hints(inherited, settings, &git.user_name, &git.user_email);
    row(ui, t.tr("prefs.identity.name.label"), |ui| {
        text_field(ui, &mut git.repo_name, &name_hint);
    });
    row(ui, t.tr("prefs.identity.email.label"), |ui| {
        text_field(ui, &mut git.repo_email, &email_hint);
        help(ui, t.tr("prefs.identity.email.help"));
    });
}

/// "This repository commits as {name} <{email}> from profile {profile}.", from the identity git
/// last read (the global one until it is read).
fn commit_as(t: Lang, id: Option<&RepoIdentity>, settings: &Settings, git: &GitConfigValues) -> String {
    use glowfly_core::repo_identity::IdentitySource;
    let (name, email, profile) = match id {
        Some(i) => {
            let profile = match i.source {
                IdentitySource::Profile(p) => settings.profile(p).map(|p| p.name.clone()),
                _ => None,
            };
            (i.name.as_str(), i.email.as_str(), profile)
        }
        None => (git.user_name.as_str(), git.user_email.as_str(), None),
    };
    if name.trim().is_empty() && email.trim().is_empty() {
        return t.tr("prefs.identity.intro.none").to_owned();
    }
    let (name, email) = (name.trim(), email.trim());
    match profile {
        Some(p) => t.trf("prefs.identity.intro.profile", &[("name", &name), ("email", &email), ("profile", &p)]),
        None => t.trf("prefs.identity.intro", &[("name", &name), ("email", &email)]),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::repo_identity::IdentitySource;
    use glowfly_core::settings::{Profile, ProfileId};

    #[test]
    fn the_identity_page_opens_with_whom_the_repository_commits_as() {
        let work = Profile { id: ProfileId(1), name: "Work".into(), ..Profile::default() };
        let settings = Settings { profiles: vec![work], ..Settings::default() };
        let git = GitConfigValues { user_name: "Ada".into(), user_email: "ada@home.example".into(), ..Default::default() };
        let id = |source| RepoIdentity { name: "Ada L".into(), email: "ada@work.example".into(), source, ..RepoIdentity::default() };
        let from_profile = id(IdentitySource::Profile(ProfileId(1)));
        assert_eq!(
            commit_as(Lang::En, Some(&from_profile), &settings, &git),
            "This repository commits as Ada L <ada@work.example> from profile Work."
        );
        let global = id(IdentitySource::Elsewhere);
        assert_eq!(commit_as(Lang::En, Some(&global), &settings, &git), "This repository commits as Ada L <ada@work.example>.");
        assert_eq!(commit_as(Lang::En, None, &settings, &git), "This repository commits as Ada <ada@home.example>.");
        let unset = RepoIdentity::default();
        assert!(commit_as(Lang::En, Some(&unset), &settings, &git).starts_with("This repository has no commit identity yet."));
    }
}
