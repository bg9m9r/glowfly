//! Switching profiles: writing a profile's identity to the global git config, then reading the
//! config back for Preferences. Folder rules: checking a folder, writing the include files and
//! the global entries, and reading (or clearing) a repository's own identity, with the identity
//! variables set in Glowfly's environment (which every git child inherits).

use super::config;
use super::execute::{Cx, Execute};
use glowfly_core::config_stamp::{self, ConfigStamp};
use glowfly_core::repo_identity::RepoIdentity;
use glowfly_core::settings::{FolderRule, ProfileId};
use glowfly_core::{identity, profile_includes, repo_identity};
use glowfly_model::msg::Msg;
use glowfly_model::profile::{ProfileEffect, ProfileMsg};
use std::path::Path;

pub(super) fn run(e: ProfileEffect) -> Option<Msg> {
    match e {
        ProfileEffect::Apply { cwd, has_repo, id, identity } => {
            let result = identity::write_global(&cwd, &identity, &[]).map(|()| config::read_git_config(&cwd, has_repo));
            Some(Msg::Profile(ProfileMsg::Applied { id, result }))
        }
        ProfileEffect::Rewrite { cwd, has_repo, id, identity } => {
            let result = identity::write_global(&cwd, &identity, &[]).map(|()| config::read_git_config(&cwd, has_repo));
            Some(Msg::Profile(ProfileMsg::Rewritten { id, result }))
        }
        ProfileEffect::CheckFolder { id, path, repository } => {
            Some(Msg::Profile(ProfileMsg::FolderChecked { id, result: profile_includes::check_folder(&path, repository) }))
        }
        ProfileEffect::SyncIncludes { cwd, plan, place } => {
            let result = match profile_includes::default_dir() {
                Some(dir) => profile_includes::sync(&cwd, &dir, &plan, place, &[]),
                None => Err("Glowfly has no settings folder.".into()),
            };
            Some(Msg::Profile(ProfileMsg::IncludesSynced(result)))
        }
        ProfileEffect::ReadIdentity { workdir, git_dir, rules, known } => {
            // Before git runs: an edit made during the read shows up in the next stamp.
            let stamp = config_stamp::of_repo(&git_dir);
            if known == Some(stamp) {
                tracing::debug!(git_dir = %git_dir.display(), "config unchanged: identity not read again");
                return None;
            }
            let result = read_identity(&workdir, &git_dir, &rules, stamp);
            Some(Msg::Profile(ProfileMsg::IdentityRead { git_dir, result }))
        }
        ProfileEffect::ClearLocalIdentity { workdir, git_dir, rules } => {
            let result = repo_identity::clear_local(&workdir, &[])
                .and_then(|()| read_identity(&workdir, &git_dir, &rules, config_stamp::of_repo(&git_dir)));
            Some(Msg::Profile(ProfileMsg::LocalCleared { git_dir, result }))
        }
    }
}

/// The identity git gives commits in `workdir`, with the environment's overrides and the config
/// files' `stamp` (taken before the read).
fn read_identity(workdir: &Path, git_dir: &Path, rules: &[(ProfileId, FolderRule)], stamp: ConfigStamp) -> Result<RepoIdentity, String> {
    let mut id = repo_identity::read(workdir, git_dir, profile_includes::default_dir().as_deref(), rules, &[])?;
    id.overrides = repo_identity::env_overrides();
    id.stamp = Some(stamp);
    Ok(id)
}

impl Execute for ProfileEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        run(self)
    }
}
