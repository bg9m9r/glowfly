//! The selection's effects (`Effect::Selection`): comparing two commits or a commit and the
//! working tree, and each selected commit's own changes.

use super::execute::{Cx, Execute, err};
use crate::effect::SelectionEffect;
use crate::msg::{Msg, SelectionMsg};
use glowfly_core::repo;

impl Execute for SelectionEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(Msg::Selection(match self {
            SelectionEffect::Compare { workdir, from, to } => {
                SelectionMsg::CompareLoaded { from, to, result: repo::compare(&workdir, from, to).map_err(err) }
            }
            SelectionEffect::WorkdirCompare { workdir, oid, generation, swapped } => SelectionMsg::WorkdirCompareLoaded {
                oid,
                generation,
                result: if swapped { repo::compare_from_workdir(&workdir, oid) } else { repo::compare_workdir(&workdir, oid) }.map_err(err),
            },
            SelectionEffect::Stacked(r) => {
                SelectionMsg::StackedLoaded { result: repo::stacked_changes(&r.workdir, &r.oids).map_err(err), oids: r.oids }
            }
        }))
    }
}
