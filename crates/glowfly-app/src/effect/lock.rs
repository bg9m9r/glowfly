//! Delete lock file's work in the runner (`Effect::Lock`, runner/lock.rs): checking a lock git
//! named after a failed job, and deleting it once confirmed (`glowfly_core::lock_file`).

#[cfg(test)]
use super::SandboxRule;
use super::meta::EffectMeta;
use crate::state::job_spec::Retry;
use crate::state::logs::EntryId;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LockEffect {
    /// Whether `lock` may be deleted (`lock_file::check`): it reads only. Reports
    /// `LockMsg::Checked`, scoped to the tab whose run `entry` failed.
    Check { workdir: PathBuf, lock: String, entry: EntryId },
    /// Checks again and deletes `lock` (`lock_file::delete`), then reports `LockMsg::Deleted`
    /// with `retry`, whichever tab is active.
    Delete { workdir: PathBuf, lock: String, retry: Box<Retry> },
}

impl EffectMeta for LockEffect {
    fn name(&self) -> &'static str {
        match self {
            LockEffect::Check { .. } => "check-lock",
            LockEffect::Delete { .. } => "delete-lock",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            LockEffect::Check { workdir, .. } => Some(workdir),
            LockEffect::Delete { .. } => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// The delete waits for the repository's other changes, so it never runs beside one.
    fn queue(&self) -> Option<&Path> {
        match self {
            LockEffect::Check { .. } => None,
            LockEffect::Delete { workdir, .. } => Some(workdir),
        }
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
