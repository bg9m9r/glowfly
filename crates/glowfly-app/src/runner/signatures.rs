//! Checking a commit's signature with git (Effect::VerifySignature).

use crate::msg::{Msg, RepoMsg};
use git2::Oid;
use glowfly_core::signature_check;
use glowfly_core::signing::SigningFormat;
use std::path::Path;

pub(super) fn verify(workdir: &Path, oid: Oid, format: SigningFormat, generation: u64) -> Msg {
    let result = signature_check::verify_commit(workdir, oid, format).map_err(|e| e.to_string());
    Msg::Repo(RepoMsg::SignatureChecked { oid, generation, result })
}
