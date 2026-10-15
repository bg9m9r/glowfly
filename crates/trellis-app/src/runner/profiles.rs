//! Switching profiles: writing a profile's identity to the global git config, then reading the
//! config back for Preferences.

use super::config;
use crate::msg::Msg;
use crate::profile::{ProfileEffect, ProfileMsg};
use trellis_core::identity;

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
    }
}
