//! The payload of `Effect::LoadStacked`: each selected commit's own changes, for a
//! multi-selection with no combined diff.

use git2::Oid;
use std::path::PathBuf;

#[derive(Debug)]
pub struct StackRequest {
    pub workdir: PathBuf,
    /// The commits, newest first (the selection's `stack_oids`).
    pub oids: Vec<Oid>,
}
