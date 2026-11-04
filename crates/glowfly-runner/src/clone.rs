//! The Clone dialog's effects (`Effect::Clone`): a sparse clone, listing the user's repositories
//! on a connected service (signing in as the pull request calls do), and checking the parent
//! folder (`files::check_clone_parent`).

use super::execute::{Cx, Execute};
use glowfly_core::{clone_ops, forge};
use glowfly_model::effect::{CloneEffect, ForgeError};
use glowfly_model::msg::{CloneMsg, Msg};

pub(super) fn run(e: CloneEffect) -> Msg {
    Msg::Clone(match e {
        CloneEffect::CheckParent { parent, delay } => return super::files::check_clone_parent(parent, delay),
        CloneEffect::Sparse { parent, cmd, dest, folders, submodules } => {
            CloneMsg::Finished(clone_ops::run_sparse(&parent, &cmd, &dest, &folders, &submodules))
        }
        CloneEffect::ListRepos { account, cwd } => {
            let result =
                forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::my_repos(&account, &a)).map_err(ForgeError::from);
            CloneMsg::ReposLoaded { account, result: Box::new(result) }
        }
    })
}

impl Execute for CloneEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
