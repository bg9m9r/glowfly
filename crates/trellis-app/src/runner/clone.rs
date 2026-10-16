//! The Clone dialog's effects (`Effect::Clone`): a sparse clone, and listing the user's
//! repositories on a connected service (signing in as the pull request calls do).

use crate::effect::{CloneEffect, ForgeError};
use crate::msg::{CloneMsg, Msg};
use trellis_core::{clone_ops, forge};

pub(super) fn run(e: CloneEffect) -> Msg {
    Msg::Clone(match e {
        CloneEffect::Sparse { parent, cmd, dest, folders } => CloneMsg::Finished(clone_ops::run_sparse(&parent, &cmd, &dest, &folders)),
        CloneEffect::ListRepos { account, cwd } => {
            let result =
                forge::resolve_auth(&account, &cwd, true, true).and_then(|a| forge::my_repos(&account, &a)).map_err(ForgeError::from);
            CloneMsg::ReposLoaded { account, result: Box::new(result) }
        }
    })
}
