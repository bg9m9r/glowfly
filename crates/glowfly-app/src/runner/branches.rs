//! The `Execute` impl of `BranchEffect`: the Delete dialog's count of the commits only the
//! branch has.

use super::execute::{Cx, Execute};
use crate::effect::BranchEffect;
use crate::msg::{BranchMsg, Msg};

impl Execute for BranchEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        match self {
            BranchEffect::CountUnmerged { workdir, name, tip } => {
                let count = glowfly_core::repo::unmerged_count(&workdir, &name, tip).map_err(|e| e.to_string());
                Some(Msg::Branch(BranchMsg::UnmergedCounted { name, tip, count }))
            }
        }
    }
}
