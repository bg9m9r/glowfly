//! The Git LFS preferences page: the status read in the background, the new-pattern buffer and
//! what to do once the status arrives; the remote LFS commands use.

use super::branches;
use trellis_core::lfs::LfsStatus;
use trellis_core::repo::Snapshot;

/// The job label of `git lfs install --local`; a successful one may offer to pull.
pub const INIT_LABEL: &str = "Initialize LFS";

#[derive(Default)]
pub struct LfsPage {
    /// None while the status is read, or when reading it failed (`error`).
    pub status: Option<LfsStatus>,
    pub error: Option<String>,
    /// Bumped by each read; an older read's result is ignored.
    pub generation: u64,
    /// The New pattern field (a view text buffer).
    pub new_pattern: String,
    /// Why the last Track was refused.
    pub pattern_error: Option<String>,
    /// The palette asked to initialize once the status allows it.
    pub init_when_ready: bool,
    /// A successful Initialize: offer to pull when the next status shows tracked patterns.
    pub offer_pull: bool,
}

/// The remote LFS push goes to: HEAD's upstream remote, else origin, else the first remote.
pub fn lfs_remote(snap: &Snapshot) -> Option<String> {
    snap.head_info().and_then(branches::remote_tracking).map(|t| t.remote.clone()).or_else(|| branches::default_remote(snap))
}
