//! The `Execute` impl of `CommitEffect`: reading a commit's detail for the details panel, and
//! checking its signature with git (signatures.rs).

use super::execute::err;
use super::execute::{Cx, Execute};
use glowfly_core::repo;
use glowfly_model::effect::CommitEffect;
use glowfly_model::msg::{Msg, RepoMsg};

impl Execute for CommitEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            CommitEffect::LoadDetail { workdir, oid } => {
                Msg::Repo(RepoMsg::DetailLoaded { oid, result: repo::commit_detail(&workdir, oid).map_err(err) })
            }
            CommitEffect::VerifySignature { workdir, oid, format, generation } => {
                super::signatures::verify(&workdir, oid, format, generation)
            }
        })
    }
}
