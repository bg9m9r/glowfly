//! The git command sequence that is running (`AppState::job`) and what follows it once it
//! succeeds.

use super::{push_pr, trust};
use std::path::PathBuf;

/// A git command sequence that is running.
pub struct RunningJob {
    pub label: String,
    /// The repository the job runs in (its tab), if any.
    pub workdir: Option<PathBuf>,
    /// Background jobs (auto-fetch) don't show toasts or announce failures.
    pub quiet: bool,
    pub clear_commit: bool,
    pub then_open: Option<(PathBuf, trust::Created)>,
    /// Run the push flow when this (commit) job succeeds.
    pub then_push: bool,
    /// Open the create pull request dialog for this branch once this (push) job succeeds.
    pub then_pr: Option<push_pr::PrStart>,
    /// Check the repository again for a pending link's target once this (fetch) job succeeds (update/deep_link_open.rs).
    pub then_link: bool,
    /// The job can't make the undo history wrong (fetch, push, or an undo/redo itself).
    pub keeps_undo: bool,
    /// The job fetches (a fetch or pull command): its success moves `Snapshot::fetched_at`.
    pub fetches: bool,
}
