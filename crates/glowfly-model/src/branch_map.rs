//! Messages and the effect of the Branch map: each local branch's drift from the main branch and
//! which pairs of branches would conflict (state/branch_map.rs, update/branch_map.rs,
//! runner/branch_map.rs, the view in ui/branch_map/). It is opened from the LOCAL section's menu
//! and the command palette; the read is libgit2 only, so nothing needs the repository's trust.

use crate::effect::partial::StopKey;
use crate::effect::{EffectMeta, SandboxRule};
use glowfly_core::branch_map::{BranchMap, Forecasts, Request};
use glowfly_core::branch_ref::BranchRef;
use glowfly_core::error::ReadFailed;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum BranchMapMsg {
    /// Show the map in place of the graph and read it.
    Open,
    /// Back to what was underneath (Close, Esc).
    Close,
    /// Read again, after a failure.
    Retry,
    /// A branch's row was chosen.
    Select(BranchRef),
    /// The read for request `generation` finished.
    Loaded { generation: u64, result: Result<Box<BranchMap>, ReadFailed> },
}

impl BranchMapMsg {
    /// Nothing is kept for an inactive tab: the intents only make sense in the active one, and a
    /// read in flight is asked for again when the tab returns.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open | Self::Close | Self::Retry | Self::Select(_) | Self::Loaded { .. } => None,
        }
    }
}

/// One read of the map (runner/branch_map.rs): libgit2 only; scoped to the repository it reads.
#[derive(Debug)]
pub struct BranchMapJob {
    pub workdir: PathBuf,
    pub request: Request,
    /// The trial merges made before, by the pair of tips.
    pub known: Arc<Forecasts>,
    pub generation: u64,
}

impl EffectMeta for BranchMapJob {
    fn name(&self) -> &'static str {
        "branch-map"
    }

    fn scope(&self) -> Option<&PathBuf> {
        Some(&self.workdir)
    }

    fn timed(&self) -> bool {
        true
    }

    /// Reads only, and writes nothing to the repository.
    fn queue(&self) -> Option<&Path> {
        None
    }

    fn stop_key(&self) -> Option<StopKey> {
        Some(StopKey::BranchMap(self.generation))
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
