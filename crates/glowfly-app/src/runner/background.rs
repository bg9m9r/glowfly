//! Work handed off by `update` (`BackgroundEffect`): a large history's graph layout, the default
//! branch's ancestry for hover highlighting, freeing replaced snapshots, and letting go of a
//! closed tab's pooled repository handles.

use super::execute::{Cx, Execute};
use crate::effect::BackgroundEffect;
use crate::msg::{GraphMsg, Msg};

pub(super) fn run(e: BackgroundEffect) -> Option<Msg> {
    match e {
        BackgroundEffect::Relayout { request, job, .. } => {
            let (fit, layout) = job.run();
            Some(Msg::Graph(GraphMsg::LaidOut { request, fit: Box::new(fit), layout: Box::new(layout) }))
        }
        BackgroundEffect::Ancestry { request, commits, tip, .. } => {
            let base = crate::state::ref_rows::reachable_in(&commits.0, tip);
            Some(Msg::Graph(GraphMsg::AncestryFound { request, base }))
        }
        BackgroundEffect::Free(d) => {
            let _span = tracing::trace_span!("free", what = d.0).entered();
            drop(d.1);
            None
        }
        BackgroundEffect::Forget(workdir) => {
            glowfly_core::repo_pool::forget(&workdir);
            None
        }
    }
}

impl Execute for BackgroundEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        run(self)
    }
}
