//! Comparing two versions of a branch (`Effect::RangeDiff`) with git range-diff; reads only.

use super::execute::{Cx, Execute};
use crate::msg::Msg;
use glowfly_model::range_diff::{RangeDiffJob, RangeDiffMsg};

pub(super) fn run(job: RangeDiffJob) -> Msg {
    let r = glowfly_core::range_diff::compare(&job.workdir, &job.ends, job.mainline);
    Msg::RangeDiff(RangeDiffMsg::Loaded { generation: job.generation, result: r.map(Box::new).map_err(super::execute::err) })
}

impl Execute for RangeDiffJob {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
