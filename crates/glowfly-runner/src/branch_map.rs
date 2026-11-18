//! Reading the Branch map (`Effect::BranchMap`): libgit2 only, so no trust check and no git
//! process; stopped through `cx.stop` when the view closes or a newer read replaces it.

use super::execute::{Cx, Execute};
use glowfly_core::branch_map::compute;
use glowfly_core::error::ReadFailed;
use glowfly_model::branch_map::{BranchMapJob, BranchMapMsg};
use glowfly_model::msg::Msg;

impl Execute for BranchMapJob {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        let stopped = || cx.stop.is_some_and(|s| s.requested());
        let result = match compute(&self.workdir, &self.request, &self.known, &stopped) {
            // Nobody waits for work that was asked to stop.
            Ok(None) => return None,
            Ok(Some(map)) => Ok(Box::new(map)),
            Err(e) => Err(ReadFailed::from(e)),
        };
        Some(Msg::BranchMap(BranchMapMsg::Loaded { generation: self.generation, result }))
    }
}
