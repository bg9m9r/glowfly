//! Messages of Delete lock file (update/lock_file.rs): the check after a failed job that decides
//! whether the button is offered, the confirmed delete, and its result.

use super::Inactive;
use crate::state::job_spec::Retry;
use crate::state::logs::EntryId;
use glowfly_core::lock_file::{Found, Refusal};
use std::path::PathBuf;

/// A lock file git named in a failed job's error (`lock`, exactly as git printed it), in the
/// repository the job ran in (`workdir`).
pub enum LockMsg {
    /// The lock was checked (runner/lock.rs) after run `entry` failed on it. `Ok(Found::Stale)`
    /// puts Delete lock file first on that run, beside its Try again.
    Checked { entry: EntryId, workdir: PathBuf, lock: String, result: Result<Found, Refusal> },
    /// Delete lock file, confirmed: check again, delete it, then run `retry`.
    Delete { workdir: PathBuf, lock: String, retry: Box<Retry> },
    /// The delete ended; on success `retry` runs.
    Deleted { lock: String, retry: Box<Retry>, result: Result<Found, Refusal> },
}

impl LockMsg {
    /// A check for a tab that is no longer active is dropped (its Try again stays); the delete's
    /// result is unscoped, so it always reaches `update`.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Checked { .. } | Self::Delete { .. } | Self::Deleted { .. } => None,
        }
    }
}
