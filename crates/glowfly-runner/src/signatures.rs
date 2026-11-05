//! Checking a commit's signature with git (`CommitEffect::VerifySignature`, runner/commit.rs).

use git2::Oid;
use glowfly_core::error::ReadFailed;
use glowfly_core::signature_check;
use glowfly_core::signing::SigningFormat;
use glowfly_model::msg::{Msg, RepoMsg};
use std::path::Path;

pub(super) fn verify(workdir: &Path, oid: Oid, format: SigningFormat, generation: u64) -> Msg {
    let result = signature_check::verify_commit(workdir, oid, format).map_err(ReadFailed::from);
    Msg::Repo(RepoMsg::SignatureChecked { oid, generation, result })
}
