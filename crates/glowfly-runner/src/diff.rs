//! The `Execute` impl of `DiffEffect`: loading a diff and finishing its highlighting
//! (highlight.rs), reading the unchanged lines between its hunks (diff_gaps.rs) and a binary
//! diff's sides, whose SVGs draw on the one SVG worker (svg_worker.rs).

use super::execute::err;
use super::execute::{Cx, Execute};
use super::{diff_gaps, highlight};
use glowfly_core::repo;
use glowfly_model::effect::DiffEffect;
use glowfly_model::msg::{DiffMsg, Msg};

impl Execute for DiffEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        match self {
            DiffEffect::Load { workdir, target, reload, generation, previous, ignore_whitespace, whole_file, listed } => {
                Some(highlight::load_job(
                    workdir,
                    target,
                    listed,
                    reload,
                    generation,
                    previous,
                    repo::LoadDiffOptions { ignore_whitespace, whole_file },
                ))
            }
            DiffEffect::Highlight { target, generation, diff, .. } => highlight::finish_job(target, generation, &diff, cx.cancelled),
            DiffEffect::Gaps { workdir, target, generation, diff } => Some(diff_gaps::load_job(workdir, target, generation, diff)),
            DiffEffect::BinarySides { workdir, target, generation } => {
                // SVG sides draw on the one SVG worker, which stops waiting past its deadline.
                let result =
                    repo::binary_sides_drawn_by(&workdir, &target, &crate::svg_worker::render).map(std::sync::Arc::new).map_err(err);
                Some(Msg::Diff(DiffMsg::BinarySidesLoaded { target, generation, result }))
            }
        }
    }
}
