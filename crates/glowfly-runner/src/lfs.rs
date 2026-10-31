//! The Git LFS effects (`Effect::Lfs`): whether git-lfs is installed, and the preferences page's
//! status.

use super::execute::{Cx, Execute, err};
use crate::effect::LfsEffect;
use crate::msg::Msg;
use glowfly_model::lfs::LfsMsg;

impl Execute for LfsEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(Msg::Lfs(match self {
            LfsEffect::Detect => LfsMsg::Detected(glowfly_core::lfs::installed()),
            LfsEffect::Load { workdir, generation } => {
                LfsMsg::Loaded { generation, result: glowfly_core::lfs::status(&workdir).map_err(err) }
            }
        }))
    }
}
