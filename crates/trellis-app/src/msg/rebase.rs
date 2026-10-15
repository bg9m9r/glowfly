//! Messages of the interactive rebase panel (update/rebase.rs).

use trellis_core::rebase::{Outcome, RebaseAction, RebaseBase, RebasePlan};

/// The interactive rebase panel: editing the list, starting the rebase, and the results
/// (update/rebase.rs).
pub enum RebaseMsg {
    /// Open the panel with the list for rebasing the current branch from `base`.
    OpenRebase(RebaseBase),
    CloseRebase,
    RebaseSetAction {
        index: usize,
        action: RebaseAction,
    },
    /// Move row `from` so it ends up at index `to` (removed, then inserted).
    RebaseMove {
        from: usize,
        to: usize,
    },
    /// Open the message editor on row `i`.
    RebaseEditMessage(usize),
    /// Update message: keep the editor's text and make the row a reword.
    RebaseSaveMessage,
    RebaseCancelMessage,
    /// The original order and actions (or, for a stale list, the list reloaded).
    RebaseReset,
    /// Start the rebase; drops ask for confirmation first (`confirmed: false`).
    StartRebase {
        confirmed: bool,
    },
    RebasePlanLoaded {
        generation: u64,
        result: Result<RebasePlan, String>,
    },
    /// The rebase job's result, for whichever tab it ran in (like `JobDone`).
    RebaseFinished {
        result: Result<Outcome, String>,
    },
}

impl RebaseMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::OpenRebase(..)
            | Self::CloseRebase
            | Self::RebaseSetAction { .. }
            | Self::RebaseMove { .. }
            | Self::RebaseEditMessage(..)
            | Self::RebaseSaveMessage
            | Self::RebaseCancelMessage
            | Self::RebaseReset
            | Self::StartRebase { .. }
            | Self::RebasePlanLoaded { .. }
            | Self::RebaseFinished { .. } => None,
        }
    }
}
