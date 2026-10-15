//! Predicting merge conflicts with git merge-tree (Effect::CheckConflicts).

use crate::conflict_check::{CheckJob, ConflictCheckMsg};
use crate::msg::Msg;

pub(super) fn run(job: CheckJob) -> Msg {
    let CheckJob { workdir, request, manual, generation } = job;
    let result = trellis_core::conflict_check::predict(&workdir, request.ours, request.target.oid).map_err(|e| e.to_string());
    Msg::ConflictCheck(ConflictCheckMsg::Checked { request, manual, generation, result })
}
