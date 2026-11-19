//! The `Execute` impl of `CommitEffect`: reading a commit's detail for the details panel, and
//! checking its signature with git (signatures.rs), and reading its whole message for Copy > Message.

use super::execute::{Cx, Execute};
use glowfly_core::error::ReadFailed;
use glowfly_core::repo;
use glowfly_model::effect::CommitEffect;
use glowfly_model::msg::{Msg, RepoMsg};

impl Execute for CommitEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(match self {
            CommitEffect::LoadDetail { workdir, oid } => {
                Msg::Repo(RepoMsg::DetailLoaded { oid, result: repo::commit_detail(&workdir, oid).map_err(ReadFailed::from) })
            }
            CommitEffect::VerifySignature { workdir, oid, format, generation } => {
                super::signatures::verify(&workdir, oid, format, generation)
            }
            CommitEffect::CopyMessage { workdir, oid } => {
                Msg::Repo(RepoMsg::MessageRead(repo::commit_message(&workdir, oid).map_err(ReadFailed::from)))
            }
        })
    }
}
