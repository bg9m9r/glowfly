//! Named profiles: seeding the first from the global identity and keeping the current one in
//! step with git config while Preferences > Profile is open, adding, renaming, editing and
//! deleting (asking first) profiles, and switching, which writes the profile's identity to the
//! global git config in the background and, once that worked, swaps in its accounts and
//! preferences (profile_setup.rs). Folder rules and the open repository's identity are in
//! profile_rules.rs. Its effects are unscoped: git config is global.

use super::prefs::save_settings;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::profile::{IdentityField, ProfileEffect, ProfileMsg};
use crate::state::profiles::{
    DELETE_BUTTON, DELETE_TITLE, GONE_AFTER_SWITCH, STILL_SWITCHING, delete_body, git_identity, rewrite_failed, switch_failed, switched,
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
            p.profiles.rules_error = None;
        }
        ProfileMsg::Add { name, copy_current, copy_accounts } => {
            let identity =
                if copy_current { st.settings.active_profile().map(|p| p.identity()).unwrap_or_default() } else { Identity::default() };
            let connections = if copy_accounts { st.settings.connections.clone() } else { Vec::new() };
            match st.settings.add_profile(&name, identity, connections) {
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
                        p.profiles.refresh(&st.settings);
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

        ProfileMsg::AskDelete(id) => {
            if st.settings.active_profile == Some(id) || st.profile_switch == Some(id) {
                return;
            }
            let Some((name, rules)) = st.settings.profile(id).map(|p| (p.name.clone(), p.folders.len())) else { return };
            let then = Msg::Profile(ProfileMsg::Delete(id));
            st.modal = Some(Modal::danger(DELETE_TITLE, delete_body(&name, rules), DELETE_BUTTON, then));
        }
        ProfileMsg::Delete(id) => {
            // The button is disabled on the switch's target; a confirmation can't outlive that.
            if st.profile_switch == Some(id) {
                return;
            }
            match st.settings.delete_profile(id) {
                Ok(removed) => {
                    fx.push(save_settings(st));
                    if !removed.folders.is_empty() {
                        super::profile_rules::request_sync(st, true, fx);
                    }
                    super::profile_setup::forget_tokens(st, removed.connections.into_iter().flatten().map(|c| c.account), fx);
                    if let Some(p) = st.prefs.as_mut() {
                        if p.profiles.renaming.as_ref().is_some_and(|f| f.id == id) {
                            p.profiles.renaming = None;
                        }
                        p.profiles.refresh(&st.settings);
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
        ProfileMsg::Rewritten { id, result } => rewritten(st, id, result, now, fx),
        ProfileMsg::AddFolder(id) => super::profile_rules::add_folder(st, id, fx),
        ProfileMsg::AddThisRepo(id) => super::profile_rules::add_this_repo(st, id, fx),
        ProfileMsg::RemoveFolder { id, path } => super::profile_rules::remove_folder(st, id, &path, fx),
        ProfileMsg::AskUseHere(id) => super::profile_rules::ask_use_here(st, id),
        ProfileMsg::UseHere { id: _, git_dir } => super::profile_rules::use_here(st, git_dir, fx),
        ProfileMsg::FolderChecked { id, result } => super::profile_rules::folder_checked(st, id, result, fx),
        ProfileMsg::IncludesSynced(result) => super::profile_rules::synced(st, result, now, fx),
        ProfileMsg::IdentityRead { git_dir, result } => super::profile_rules::identity_read(st, git_dir, result, fx),
        ProfileMsg::LocalCleared { git_dir, result } => super::profile_rules::local_cleared(st, git_dir, result, now, fx),
    }
}

/// Fresh git config reached the open Profile page: seed the first profile or bring the current
/// one in step with git, then fill the page. Not while a switch writes the config: a read then
/// may hold some of the other profile's values, and the current profile would take them.
pub(super) fn on_config(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(git) = st.prefs.as_ref().and_then(|p| p.git.as_ref()).map(git_identity) else { return };
    if st.profile_switch.is_none() && st.settings.seed_or_sync_profiles(&git) {
        fx.push(save_settings(st));
    }
    if let Some(p) = st.prefs.as_mut() {
        p.profiles.fill(&st.settings, None);
    }
}

/// Stores a field the Profile page saves (prefs_save.rs). The current profile's identity is
/// git's, so its changes are written to the global config as well; clearing its key only
/// forgets it here. While a switch runs, that write waits for it (two writers would fight over
/// the file, and the switch's would win or lose at random): the profile that ends up current
/// gets its identity written afterwards.
pub(super) fn set_identity(st: &mut AppState, id: ProfileId, field: IdentityField, value: &str, fx: &mut Vec<Effect>) {
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
    // Its include file has the identity; the entries stay where they are.
    if st.settings.profile(id).is_some_and(|p| !p.folders.is_empty()) {
        super::profile_rules::request_sync(st, false, fx);
    }
    if st.profile_switch.is_some() {
        if (st.settings.active_profile == Some(id) || st.profile_switch == Some(id)) && !st.profile_switch_edits.contains(&id) {
            st.profile_switch_edits.push(id);
        }
        return;
    }
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
    let name = st.settings.profile(id).map(|p| p.name.clone());
    let failed = result.is_err();
    match result {
        Ok(values) => {
            match &name {
                Some(name) => {
                    // The accounts and preferences switch with the identity.
                    let swap = st.settings.swap_profile_setup(id);
                    st.settings.active_profile = Some(id);
                    fx.push(save_settings(st));
                    super::profile_setup::switched(st, swap, now, fx);
                    st.toast(true, switched(name), now);
                    super::profile_rules::want_identity(st, fx);
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
        }
    }
    // A failed switch put git's earlier values back; the read shows them.
    if !finish_switch(st, fx) && failed && st.prefs.is_some() {
        fx.push(Effect::ReadGitConfig { cwd: st.any_dir(), has_repo: st.snap.is_some() });
    }
}

/// The current profile's identity, edited during a switch, was written (or not).
fn rewritten(
    st: &mut AppState,
    id: ProfileId,
    result: Result<crate::state::prefs::GitConfigValues, String>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let failed = result.is_err();
    match result {
        Ok(values) => {
            if let Some(p) = st.prefs.as_mut() {
                p.git = Some(values);
                p.profiles.refresh(&st.settings);
            }
        }
        Err(e) => {
            let name = st.settings.profile(id).map(|p| p.name.clone()).unwrap_or_else(|| "(deleted)".into());
            let text = rewrite_failed(&name, &e);
            st.log(false, text.clone());
            st.toast(false, text, now);
        }
    }
    if !finish_switch(st, fx) && failed && st.prefs.is_some() {
        fx.push(Effect::ReadGitConfig { cwd: st.any_dir(), has_repo: st.snap.is_some() });
    }
}

/// Ends the running switch's writes. When the profile that is now current was edited
/// meanwhile, its identity is written next (the switch stays busy until then); returns
/// whether it was.
fn finish_switch(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    st.profile_switch = None;
    let edits = std::mem::take(&mut st.profile_switch_edits);
    let Some(current) = st.settings.active_profile().filter(|p| edits.contains(&p.id)) else {
        // A folder-rule sync asked for during the switch can write the config now.
        super::profile_rules::run_deferred(st, fx);
        return false;
    };
    let (id, identity) = (current.id, current.identity());
    st.profile_switch = Some(id);
    fx.push(Effect::Profile(ProfileEffect::Rewrite { cwd: st.any_dir(), has_repo: st.snap.is_some(), id, identity }));
    true
}
