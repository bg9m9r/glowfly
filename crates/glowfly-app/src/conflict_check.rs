//! Messages and the effect's job of predicting merge conflicts of the checked-out branch
//! (state/conflict_check.rs, update/conflict_check.rs, runner/conflict_check.rs, the toolbar
//! chip and report in ui/conflict_check.rs, the branch menus in ui/branch_menu.rs).

use git2::Oid;
use glowfly_core::conflict_check::{MergeTarget, Prediction};
use std::path::PathBuf;

/// One check: merge `target` into the branch `branch`, whose tip is `ours`.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckRequest {
    /// The branch merged into (shown in the texts).
    pub branch: String,
    pub ours: Oid,
    pub target: MergeTarget,
}

/// What Effect::CheckConflicts runs.
#[derive(Debug)]
pub struct CheckJob {
    pub workdir: PathBuf,
    pub request: CheckRequest,
    /// Asked for by the user: the result opens a dialog. Automatic checks feed the chip.
    pub manual: bool,
    /// `ConflictChecks`' generation when an automatic check started (0 for manual ones).
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConflictCheckMsg {
    /// Check the checked-out branch against its target now and show the result (head branch
    /// menu, the popover's Check again).
    CheckNow,
    /// Check merging local or remote-tracking branch .0 into the checked-out branch (that
    /// branch's menu).
    CheckBranch(String),
    /// A check ended.
    Checked { request: CheckRequest, manual: bool, generation: u64, result: Result<Prediction, String> },
}

impl ConflictCheckMsg {
    /// Nothing is kept for an inactive tab: a dropped Checked was abandoned by parking, and the
    /// tab asks again when it becomes active.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::CheckNow | Self::CheckBranch(_) | Self::Checked { .. } => None,
        }
    }
}
