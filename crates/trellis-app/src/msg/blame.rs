//! Messages of the blame view (update/blame.rs).

use super::Inactive;
use crate::state::blame::BlameData;
use git2::Oid;

/// The blame view (update/blame.rs).
pub enum BlameMsg {
    /// Blame `path` at HEAD (`at: None`) or as of commit `at`.
    Open {
        path: String,
        at: Option<Oid>,
    },
    Close,
    /// A run's gutter was clicked: select its commit on the graph and close the view.
    SelectCommit(Oid),
    Loaded {
        path: String,
        generation: u64,
        result: Result<Box<BlameData>, String>,
    },
}

impl BlameMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// the view asks again when its tab returns.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Open { .. } | Self::Close | Self::SelectCommit(..) | Self::Loaded { .. } => None,
        }
    }
}
