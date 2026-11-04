//! `NetworkEffect`'s `Execute` impl: a hosting service's calls (forge_calls.rs) and avatar
//! downloads (`glowfly_core::avatars`), one after another on the effect's worker.

use super::execute::{Cx, Execute};
use super::forge_calls;
use glowfly_core::avatars;
use glowfly_model::effect::NetworkEffect;
use glowfly_model::msg::{AvatarMsg, ForgeMsg, Msg};

impl Execute for NetworkEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        match self {
            NetworkEffect::Forge { target, cwd, auth, interactive, connected, call } => {
                Some(Msg::Forge(ForgeMsg::ForgeReplied(forge_calls::call(target, &cwd, auth, interactive, connected, call))))
            }
            NetworkEffect::Avatars(emails) => {
                let images = emails
                    .into_iter()
                    .map(|e| {
                        let img = avatars::fetch(&e);
                        (e, img)
                    })
                    .collect();
                Some(Msg::Avatars(AvatarMsg::AvatarsLoaded(images)))
            }
        }
    }
}
