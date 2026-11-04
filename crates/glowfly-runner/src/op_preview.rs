//! Working out what an operation would do (Effect::OpPreview); reads only.

use super::execute::{Cx, Execute};
use glowfly_model::effect::OpPreviewJob;
use glowfly_model::msg::{Msg, OpPreviewMsg};

pub(super) fn run(job: OpPreviewJob) -> Msg {
    let result = glowfly_core::op_preview::preview(&job.workdir, &job.request).map(Box::new).map_err(|e| e.to_string());
    Msg::OpPreview(OpPreviewMsg::Loaded { job, result })
}

impl Execute for OpPreviewJob {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
