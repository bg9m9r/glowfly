//! Repository trust's reads (`Effect::Trust`): resolving and scanning the repositories Glowfly
//! just made from a trusted one, so they can be trusted on the same terms.

use super::execute::{Cx, Execute};
use crate::effect::TrustEffect;
use crate::msg::{InheritedRepo, Msg, TrustMsg};

pub(super) fn run(e: TrustEffect) -> Msg {
    match e {
        TrustEffect::Inherit { kind, from, paths, open } => {
            let repos = paths
                .into_iter()
                .map(|path| {
                    let real = glowfly_core::paths::resolve(&path);
                    let print = glowfly_core::trust::scan_at(&real).ok().map(|s| s.print());
                    InheritedRepo { path, real, print }
                })
                .collect();
            Msg::Trust(TrustMsg::Inherited { kind, from, repos, open })
        }
    }
}

impl Execute for TrustEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
