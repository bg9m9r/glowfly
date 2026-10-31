//! The file history's request for a file's text (`Effect::LoadFileText`).

use super::EffectMeta;
use super::SandboxRule;
use git2::Oid;
use std::path::{Path, PathBuf};

/// Read `path` of `workdir` as of commit `oid`.
#[derive(Debug)]
pub struct FileTextRequest {
    pub workdir: PathBuf,
    pub oid: Oid,
    pub path: String,
    /// Read the version before `oid` (which deletes the file).
    pub before: bool,
    /// Give a Git LFS pointer's content; set only when git may run in the repository unasked.
    pub smudge: bool,
}

impl EffectMeta for FileTextRequest {
    fn name(&self) -> &'static str {
        "file-text"
    }

    fn scope(&self) -> Option<&PathBuf> {
        Some(&self.workdir)
    }

    fn timed(&self) -> bool {
        true
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
