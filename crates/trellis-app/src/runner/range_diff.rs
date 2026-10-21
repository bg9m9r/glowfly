//! Comparing two versions of a branch (`Effect::RangeDiff`) with git range-diff; reads only.

use crate::msg::Msg;
use crate::range_diff::{RangeDiffJob, RangeDiffMsg};

pub(super) fn run(job: RangeDiffJob) -> Msg {
    let r = trellis_core::range_diff::compare(&job.workdir, &job.ends, job.mainline);
    Msg::RangeDiff(RangeDiffMsg::Loaded { generation: job.generation, result: r.map(Box::new).map_err(super::execute::err) })
}
