//! Profile folder rules (Preferences > Profile > Folders): adding a picked folder or the open
//! repository after checking it on disk, removing one, and keeping the include files and the
//! global config's entries in step (one sync at a time, never during a switch). The open
//! repository's effective identity is read from git on open, focus, closing Preferences and
//! after a sync or a switch; when a rule should apply but git gives the global file, the
//! entries are placed again once per git dir and session (a `[user]` section written after
//! them). Using a profile in a repository that sets its own identity removes that identity,
//! after a danger confirmation only.

use super::prefs::save_settings;
use crate::effect::Effect;
use crate::msg::{FolderPurpose, Msg};
use crate::profile::{ProfileEffect, ProfileMsg};
use crate::state::repo_identity::{USE_HERE_TITLE, local_who, needs_repair, use_here_body, use_here_button};
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::profile_includes;
use trellis_core::repo_identity::RepoIdentity;
use trellis_core::settings::{FolderRule, ProfileId};

/// Add folder…: the folder picker, for a profile that exists.
pub(super) fn add_folder(st: &mut AppState, id: ProfileId, fx: &mut Vec<Effect>) {
    if st.settings.profile(id).is_some() {
        let title = crate::state::profile_rules::PICK_FOLDER_TITLE.into();
        fx.push(Effect::PickFolder { title, purpose: FolderPurpose::ProfileFolder(id) });
    }
}

pub(super) fn folder_picked(_st: &mut AppState, id: ProfileId, path: PathBuf, fx: &mut Vec<Effect>) {
    fx.push(Effect::Profile(ProfileEffect::CheckFolder { id, path, repository: false }));
}

/// Use for this repository: the open repository's common git dir, so its worktrees count too.
pub(super) fn add_this_repo(st: &mut AppState, id: ProfileId, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    fx.push(Effect::Profile(ProfileEffect::CheckFolder { id, path: snap.common.dir.clone(), repository: true }));
}

pub(super) fn folder_checked(st: &mut AppState, id: ProfileId, result: Result<FolderRule, String>, fx: &mut Vec<Effect>) {
    let problem = match result.map(|rule| st.settings.add_folder_rule(id, rule)) {
        Ok(Ok(())) => None,
        Ok(Err(problem)) => Some(problem.message()),
        Err(e) => Some(e),
    };
    if problem.is_none() {
        fx.push(save_settings(st));
        request_sync(st, true, fx);
    }
    if let Some(p) = st.prefs.as_mut() {
        p.profiles.rules_error = problem;
    }
}

pub(super) fn remove_folder(st: &mut AppState, id: ProfileId, path: &str, fx: &mut Vec<Effect>) {
    if st.settings.remove_folder_rule(id, path) {
        fx.push(save_settings(st));
        request_sync(st, true, fx);
    }
}

/// Writes the include files (and with `place` the entries) now, or once the running sync or
/// switch is done.
pub(super) fn request_sync(st: &mut AppState, place: bool, fx: &mut Vec<Effect>) {
    if st.includes.running || st.profile_switch.is_some() {
        st.includes.again = Some(st.includes.again.unwrap_or(false) || place);
        return;
    }
    st.includes.running = true;
    let plan = profile_includes::plan(&st.settings);
    fx.push(Effect::Profile(ProfileEffect::SyncIncludes { cwd: st.any_dir(), plan, place }));
}

/// Starts a sync that was asked for while another ran or a switch wrote the config.
pub(super) fn run_deferred(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(place) = st.includes.again.take() {
        request_sync(st, place, fx);
    }
}

pub(super) fn synced(st: &mut AppState, result: Result<(), String>, now: Instant, fx: &mut Vec<Effect>) {
    st.includes.running = false;
    if let Err(e) = &result {
        tracing::warn!("{e}");
        st.log(false, e.clone());
        st.toast(false, e.clone(), now);
    }
    st.includes.error = result.err();
    want_identity(st, fx);
    // A switch chosen meanwhile goes first; a sync asked for since then waits for it.
    if let Some(id) = st.includes.held_switch.take() {
        super::profiles::start_switch(st, id, fx);
    }
    run_deferred(st, fx);
}

/// Reads the active repository's effective identity from git.
pub(super) fn want_identity(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let (workdir, git_dir) = (snap.workdir.clone(), snap.git_dir.clone());
    fx.push(Effect::Profile(ProfileEffect::ReadIdentity { workdir, git_dir, rules: st.settings.rules() }));
}

/// Stores a read; a rule git didn't apply gets its entries placed again, once per git dir.
pub(super) fn identity_read(st: &mut AppState, git_dir: PathBuf, result: Result<RepoIdentity, String>, fx: &mut Vec<Effect>) {
    let id = match result {
        Ok(id) => id,
        Err(e) => {
            tracing::debug!("{e}");
            st.identities.remove(&git_dir);
            return;
        }
    };
    let repair = needs_repair(&id, &st.settings).is_some() && !st.includes.healed.contains(&git_dir);
    st.identities.insert(git_dir.clone(), id);
    if repair {
        // Most likely a [user] section written after the entries, or a global config that
        // lacks them (settings copied from another machine); the read after the sync tells.
        st.includes.healed.insert(git_dir);
        request_sync(st, true, fx);
    }
}

/// Use <profile> here: ask before removing the repository's own identity.
pub(super) fn ask_use_here(st: &mut AppState, id: ProfileId) {
    let Some(snap) = &st.snap else { return };
    let Some(name) = st.settings.profile(id).map(|p| p.name.clone()) else { return };
    let Some(identity) = st.identities.get(&snap.git_dir) else { return };
    let then = Msg::Profile(ProfileMsg::UseHere { id, git_dir: snap.git_dir.clone() });
    st.modal = Some(Modal::danger(USE_HERE_TITLE, use_here_body(&local_who(identity), &name), use_here_button(&name), then));
}

/// The confirmation was accepted: remove the identity of the repository it named, if that is
/// still the one open.
pub(super) fn use_here(st: &mut AppState, git_dir: PathBuf, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref().filter(|s| s.git_dir == git_dir) else { return };
    let workdir = snap.workdir.clone();
    fx.push(Effect::Profile(ProfileEffect::ClearLocalIdentity { workdir, git_dir, rules: st.settings.rules() }));
}

pub(super) fn local_cleared(st: &mut AppState, git_dir: PathBuf, result: Result<RepoIdentity, String>, now: Instant, fx: &mut Vec<Effect>) {
    match result {
        Ok(id) => {
            st.identities.insert(git_dir, id);
            // Repo > Identity shows the repository's own values.
            if st.prefs.is_some() {
                fx.push(Effect::ReadGitConfig { cwd: st.any_dir(), has_repo: st.snap.is_some() });
            }
        }
        Err(e) => {
            tracing::warn!("{e}");
            st.log(false, e.clone());
            st.toast(false, e, now);
        }
    }
}
