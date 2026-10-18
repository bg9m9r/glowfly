//! The file history's request for a file's text (`Effect::LoadFileText`).

use git2::Oid;
use std::path::PathBuf;

/// Read `path` of `workdir` as of commit `oid`.
#[derive(Debug)]
pub struct FileTextRequest {
    pub workdir: PathBuf,
    pub oid: Oid,
    pub path: String,
}
