//! Checking a commit's signature with git (Effect::VerifySignature).

use crate::msg::{Msg, RepoMsg};
use git2::Oid;
use std::path::Path;
use trellis_core::signature_check;
use trellis_core::signing::SigningFormat;

pub(super) fn verify(workdir: &Path, oid: Oid, format: SigningFormat, generation: u64) -> Msg {
    let result = signature_check::verify_commit(workdir, oid, format).map_err(|e| e.to_string());
    Msg::Repo(RepoMsg::SignatureChecked { oid, generation, result })
}
