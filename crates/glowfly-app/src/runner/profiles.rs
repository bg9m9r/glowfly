//! Switching profiles: writing a profile's identity to the global git config, then reading the
//! config back for Preferences. Folder rules: checking a folder, writing the include files and
//! the global entries, and reading (or clearing) a repository's own identity.

use super::config;
use crate::msg::Msg;
use crate::profile::{ProfileEffect, ProfileMsg};
use glowfly_core::{identity, profile_includes, repo_identity};

pub(super) fn run(e: ProfileEffect) -> Msg {
    match e {
        ProfileEffect::Apply { cwd, has_repo, id, identity } => {
            let result = identity::write_global(&cwd, &identity, &[]).map(|()| config::read_git_config(&cwd, has_repo));
            Msg::Profile(ProfileMsg::Applied { id, result })
        }
        ProfileEffect::Rewrite { cwd, has_repo, id, identity } => {
            let result = identity::write_global(&cwd, &identity, &[]).map(|()| config::read_git_config(&cwd, has_repo));
            Msg::Profile(ProfileMsg::Rewritten { id, result })
        }
        ProfileEffect::CheckFolder { id, path, repository } => {
            Msg::Profile(ProfileMsg::FolderChecked { id, result: profile_includes::check_folder(&path, repository) })
        }
        ProfileEffect::SyncIncludes { cwd, plan, place } => {
            let result = match profile_includes::default_dir() {
                Some(dir) => profile_includes::sync(&cwd, &dir, &plan, place, &[]),
                None => Err("Trellis has no settings folder.".into()),
            };
            Msg::Profile(ProfileMsg::IncludesSynced(result))
        }
        ProfileEffect::ReadIdentity { workdir, git_dir, rules } => {
            let result = repo_identity::read(&workdir, &git_dir, profile_includes::default_dir().as_deref(), &rules, &[]);
            Msg::Profile(ProfileMsg::IdentityRead { git_dir, result })
        }
        ProfileEffect::ClearLocalIdentity { workdir, git_dir, rules } => {
            let result = repo_identity::clear_local(&workdir, &[])
                .and_then(|()| repo_identity::read(&workdir, &git_dir, profile_includes::default_dir().as_deref(), &rules, &[]));
            Msg::Profile(ProfileMsg::LocalCleared { git_dir, result })
        }
    }
}
