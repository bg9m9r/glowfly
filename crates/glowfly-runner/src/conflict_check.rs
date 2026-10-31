//! Predicting merge conflicts with git merge-tree (Effect::CheckConflicts).

use super::execute::{Cx, Execute};
use crate::msg::Msg;
use glowfly_model::conflict_check::{CheckJob, ConflictCheckMsg};

pub(super) fn run(job: CheckJob) -> Msg {
    let CheckJob { workdir, request, manual, generation } = job;
    let result = glowfly_core::conflict_check::predict(&workdir, request.ours, request.target.oid).map_err(|e| e.to_string());
    Msg::ConflictCheck(ConflictCheckMsg::Checked { request, manual, generation, result })
}

impl Execute for CheckJob {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
