//! Named identity profiles: seeding the first from the global identity and keeping the current
//! one in step with git config while Preferences > Profile is open, adding, renaming, editing
//! and deleting (asking first) profiles, and switching, which writes the profile's identity to
//! the global git config in the background. Its effects are unscoped: git config is global.

use super::prefs::save_settings;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::profile::{IdentityField, ProfileEffect, ProfileMsg};
use crate::state::profiles::{
    DELETE_BUTTON, DELETE_TITLE, GONE_AFTER_SWITCH, STILL_SWITCHING, delete_body, git_identity, switch_failed, switched,
};
use crate::state::{AppState, Modal, PrefPage};
use std::time::Instant;
use trellis_core::identity::Identity;
use trellis_core::settings::{ProfileId, ProfileProblem};

pub(super) fn handle(st: &mut AppState, m: ProfileMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        ProfileMsg::ShowPage => {
            let Some(p) = st.prefs.as_mut() else { return };
            p.page = PrefPage::Profile;
            if p.git.is_some() {
                on_config(st, fx);
            }
        }
        ProfileMsg::Select(id) => {
            let Some(p) = st.prefs.as_mut() else { return };
            p.profiles.fill(&st.settings, Some(id));
            p.profiles.renaming = None;
        }
        ProfileMsg::Add { name, copy_current } => {
            let identity =
                if copy_current { st.settings.active_profile().map(|p| p.identity()).unwrap_or_default() } else { Identity::default() };
            match st.settings.add_profile(&name, identity) {
                Ok(id) => {
                    if let Some(p) = st.prefs.as_mut() {
                        p.profiles.adding = None;
                        p.profiles.fill(&st.settings, Some(id));
                    }
                    fx.push(save_settings(st));
                }
                Err(problem) => {
                    if let Some(form) = st.prefs.as_mut().and_then(|p| p.profiles.adding.as_mut()) {
                        form.error = Some(problem.message().into());
                    }
                }
            }
        }
        ProfileMsg::Rename { id, name } => {
            let before = st.settings.profile(id).map(|p| p.name.clone());
            match st.settings.rename_profile(id, &name) {
                Ok(()) => {
                    if let Some(p) = st.prefs.as_mut() {
                        p.profiles.renaming = None;
                        p.profiles.fill(&st.settings, None);
                    }
                    if st.settings.profile(id).map(|p| p.name.clone()) != before {
                        fx.push(save_settings(st));
                    }
                }
                Err(problem) => {
                    if let Some(form) = st.prefs.as_mut().and_then(|p| p.profiles.renaming.as_mut()).filter(|f| f.id == id) {
                        form.error = Some(problem.message().into());
                    }
                }
            }
        }
        ProfileMsg::SetIdentity { id, field, value } => set_identity(st, id, field, value.trim(), fx),
        ProfileMsg::AskDelete(id) => {
            if st.settings.active_profile == Some(id) || st.profile_switch == Some(id) {
                return;
            }
            let Some(name) = st.settings.profile(id).map(|p| p.name.clone()) else { return };
            st.modal = Some(Modal::Confirm {
                title: DELETE_TITLE.into(),
                body: delete_body(&name),
                button: DELETE_BUTTON.into(),
                then: Box::new(Msg::Profile(ProfileMsg::Delete(id))),
            });
        }
        ProfileMsg::Delete(id) => {
            // The button is disabled on the switch's target; a confirmation can't outlive that.
            if st.profile_switch == Some(id) {
                return;
            }
            match st.settings.delete_profile(id) {
                Ok(_) => {
                    fx.push(save_settings(st));
                    if let Some(p) = st.prefs.as_mut() {
                        if p.profiles.renaming.as_ref().is_some_and(|f| f.id == id) {
                            p.profiles.renaming = None;
                        }
                        p.profiles.fill(&st.settings, None);
                    }
                }
                Err(problem) => st.log(false, problem.message()),
            }
        }
        ProfileMsg::Switch(id) => {
            if st.profile_switch.is_some() {
                st.log(false, STILL_SWITCHING);
                return;
            }
            let Some(identity) = st.settings.profile(id).map(|p| p.identity()) else {
                st.log(false, ProfileProblem::Missing.message());
                return;
            };
            st.profile_switch = Some(id);
            fx.push(Effect::Profile(ProfileEffect::Apply { cwd: st.any_dir(), has_repo: st.snap.is_some(), id, identity }));
        }
        ProfileMsg::Applied { id, result } => applied(st, id, result, now, fx),
    }
}

/// Fresh git config reached the open Profile page: seed the first profile or bring the current
/// one in step with git, then fill the page.
pub(super) fn on_config(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(git) = st.prefs.as_ref().and_then(|p| p.git.as_ref()).map(git_identity) else { return };
    if st.settings.seed_or_sync_profiles(&git) {
        fx.push(save_settings(st));
    }
    if let Some(p) = st.prefs.as_mut() {
        p.profiles.fill(&st.settings, None);
    }
}

/// Stores an edited field. The current profile's identity is git's, so its changes are written
/// to the global config as well; clearing its key only forgets it here.
fn set_identity(st: &mut AppState, id: ProfileId, field: IdentityField, value: &str, fx: &mut Vec<Effect>) {
    let Some(profile) = st.settings.profile_mut(id) else { return };
    let changed = match field {
        IdentityField::AuthorName => replace(&mut profile.author_name, value.to_owned()),
        IdentityField::AuthorEmail => replace(&mut profile.author_email, value.to_owned()),
        IdentityField::SigningKey => replace(&mut profile.signing_key, Some(value.to_owned()).filter(|v| !v.is_empty())),
    };
    if !changed {
        return;
    }
    fx.push(save_settings(st));
    if st.settings.active_profile != Some(id) || (field == IdentityField::SigningKey && value.is_empty()) {
        return;
    }
    let git_value = Some(value.to_owned()).filter(|v| !v.is_empty());
    fx.push(Effect::SetGitConfig { cwd: st.any_dir(), global: true, key: field.git_key().into(), value: git_value });
    if let Some(git) = st.prefs.as_mut().and_then(|p| p.git.as_mut()) {
        let mirror = match field {
            IdentityField::AuthorName => &mut git.user_name,
            IdentityField::AuthorEmail => &mut git.user_email,
            IdentityField::SigningKey => &mut git.signing_key,
        };
        *mirror = value.to_owned();
    }
}

/// Sets `slot` to `value`; returns whether that changed it.
fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        return false;
    }
    *slot = value;
    true
}

/// A switch's writes finished: on success the profile becomes current, and the open
/// Preferences shows the config as it now is.
fn applied(
    st: &mut AppState,
    id: ProfileId,
    result: Result<crate::state::prefs::GitConfigValues, String>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    st.profile_switch = None;
    let name = st.settings.profile(id).map(|p| p.name.clone());
    match result {
        Ok(values) => {
            match &name {
                Some(name) => {
                    st.settings.active_profile = Some(id);
                    fx.push(save_settings(st));
                    st.toast(true, switched(name), now);
                }
                None => st.log(false, GONE_AFTER_SWITCH),
            }
            if let Some(p) = st.prefs.as_mut() {
                p.git = Some(values);
                p.profiles.fill(&st.settings, name.is_some().then_some(id));
            }
        }
        Err(e) => {
            let text = switch_failed(name.as_deref().unwrap_or("(deleted)"), &e);
            st.log(false, text.clone());
            st.toast(false, text, now);
            if st.prefs.is_some() {
                fx.push(Effect::ReadGitConfig { cwd: st.any_dir(), has_repo: st.snap.is_some() });
            }
        }
    }
}
