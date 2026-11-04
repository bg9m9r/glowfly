//! The selection's effects (`Effect::Selection`): comparing two commits or a commit and the
//! working tree, and each selected commit's own changes.

use super::execute::{Cx, Execute};
use glowfly_core::error::ReadFailed;
use glowfly_core::repo;
use glowfly_model::effect::SelectionEffect;
use glowfly_model::msg::{Msg, SelectionMsg};

impl Execute for SelectionEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(Msg::Selection(match self {
            SelectionEffect::Compare { workdir, from, to } => {
                SelectionMsg::CompareLoaded { from, to, result: repo::compare(&workdir, from, to).map_err(ReadFailed::from) }
            }
            SelectionEffect::WorkdirCompare { workdir, oid, generation, swapped } => SelectionMsg::WorkdirCompareLoaded {
                oid,
                generation,
                result: if swapped { repo::compare_from_workdir(&workdir, oid) } else { repo::compare_workdir(&workdir, oid) }
                    .map_err(ReadFailed::from),
            },
            SelectionEffect::Stacked(r) => {
                SelectionMsg::StackedLoaded { result: repo::stacked_changes(&r.workdir, &r.oids).map_err(ReadFailed::from), oids: r.oids }
            }
        }))
    }
}
