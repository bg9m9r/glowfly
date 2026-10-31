//! Effects of the SUBMODULES section: reading a submodule (or all of them, before a batch update)
//! again before an update or a removal is confirmed, checking the Add dialog's folder on disk, and removing a submodule.

use super::EffectMeta;
use super::SandboxRule;
use crate::submodule::{Batch, RemoveRequest};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum SubmoduleEffect {
    /// Reads the submodule at `path` (repo-relative) again, with the commits an update would
    /// leave behind, before confirming the update. Reported as `SubmoduleMsg::Checked`.
    Check { workdir: PathBuf, path: String },
    /// Whether git can add a submodule at the repo path `path` in `repo`, checked after `delay`
    /// so typing doesn't wait on the disk. Reported as `SubmoduleMsg::AddPathChecked`.
    CheckAddPath { repo: PathBuf, path: String, delay: Duration },
    /// Reads what removing the submodule at `path` would delete. Reported as
    /// `SubmoduleMsg::RemoveChecked`.
    CheckRemove { workdir: PathBuf, path: String },
    /// Reads every submodule again before a batch update. Reported as `SubmoduleMsg::CheckedAll`;
    /// `once_finished`: nothing is reported while an operation is still in progress there (a
    /// sequence with picks left).
    CheckAll { workdir: PathBuf, batch: Batch, once_finished: bool },
    /// Deinitializes the submodule, removes it with `git rm` and deletes its stored module. It
    /// ends the active job (`SubmoduleMsg::Removed`).
    Remove(RemoveRequest),
}

impl EffectMeta for SubmoduleEffect {
    fn name(&self) -> &'static str {
        match self {
            SubmoduleEffect::Check { .. } => "check-submodule",
            SubmoduleEffect::CheckAddPath { .. } => "check-submodule-path",
            SubmoduleEffect::CheckRemove { .. } => "check-submodule-remove",
            SubmoduleEffect::CheckAll { .. } => "check-submodules",
            SubmoduleEffect::Remove(_) => "remove-submodule",
        }
    }

    /// The tab the result belongs to. The Add dialog's check names its repository itself, and a
    /// removal ends the active job whichever tab is active, so both are unscoped.
    fn scope(&self) -> Option<&PathBuf> {
        match self {
            SubmoduleEffect::Check { workdir, .. }
            | SubmoduleEffect::CheckRemove { workdir, .. }
            | SubmoduleEffect::CheckAll { workdir, .. } => Some(workdir),
            SubmoduleEffect::CheckAddPath { .. } | SubmoduleEffect::Remove(_) => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// Removing a submodule changes the repository; the checks only read.
    fn queue(&self) -> Option<&Path> {
        match self {
            SubmoduleEffect::Remove(r) => Some(&r.repo),
            SubmoduleEffect::Check { .. }
            | SubmoduleEffect::CheckAddPath { .. }
            | SubmoduleEffect::CheckRemove { .. }
            | SubmoduleEffect::CheckAll { .. } => None,
        }
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
