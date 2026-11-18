//! Messages of Delete lock file (update/lock_file.rs) and of Try again for a working-tree op
//! (update/lock_local.rs): a working-tree op that failed on a lock, the check after a failure
//! that decides whether the button is offered, the confirmed delete, and its result.

use super::{FailedWrite, Inactive};
use crate::effect::LocalOp;
use crate::state::lock_retry::{LocalRetry, LockRetry};
use crate::state::logs::EntryId;
use glowfly_core::error::WriteFailed;
use glowfly_core::lock_file::{Found, Refusal};
use std::path::PathBuf;

/// A lock file git or libgit2 named in a failed job's or working-tree op's error (`lock`,
/// exactly as it was printed), in the repository it ran in (`workdir`).
pub enum LockMsg {
    /// A working-tree op that may run again (`LocalOp::retryable`) failed on lock `lock`
    /// (runner/git_ops.rs): Try again is offered, and the lock checked.
    LocalLocked { workdir: PathBuf, op: Box<LocalOp>, lock: String, error: WriteFailed },
    /// The lock was checked (runner/lock.rs) after entry `entry` failed on it. `Ok(Found::Stale)`
    /// puts Delete lock file first on that entry, beside its Try again.
    Checked { entry: EntryId, workdir: PathBuf, lock: String, result: Result<Found, Refusal> },
    /// Delete lock file, confirmed: check again, delete it, then run `retry`.
    Delete { workdir: PathBuf, lock: String, retry: LockRetry },
    /// The delete ended; on success `retry` runs.
    Deleted { lock: String, retry: LockRetry, result: Result<Found, Refusal> },
    /// Try again for a working-tree op that failed on a lock.
    Again(Box<LocalRetry>),
}

impl LockMsg {
    /// A working-tree op's lock failure for a parked tab is logged there, without buttons. A
    /// check for a tab that is no longer active is dropped (its Try again stays); the delete's
    /// result and Try again are unscoped, so they always reach `update`.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::LocalLocked { error, .. } => Some(Inactive::WriteFailed { at: FailedWrite::LocalOp, error, undo: None }),
            Self::Checked { .. } | Self::Delete { .. } | Self::Deleted { .. } | Self::Again(_) => None,
        }
    }
}
