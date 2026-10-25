//! The Git LFS effects (`Effect::Lfs`): whether git-lfs is installed, and the preferences page's
//! status.

use super::execute::{Cx, Execute, err};
use crate::effect::LfsEffect;
use crate::lfs::LfsMsg;
use crate::msg::Msg;

impl Execute for LfsEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(Msg::Lfs(match self {
            LfsEffect::Detect { cwd } => LfsMsg::Detected(glowfly_core::lfs::installed(&cwd)),
            LfsEffect::Load { workdir, generation } => {
                LfsMsg::Loaded { generation, result: glowfly_core::lfs::status(&workdir).map_err(err) }
            }
        }))
    }
}
