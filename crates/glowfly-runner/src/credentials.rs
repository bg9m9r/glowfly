//! Saving and dropping HTTPS sign-ins with git's credential helpers (`CredentialEffect`).

use super::execute::{Cx, Execute};
use glowfly_core::git_credential;
use glowfly_model::effect::CredentialEffect;
use glowfly_model::msg::{CredentialMsg, Msg};

pub(super) fn run(e: CredentialEffect) -> Msg {
    let msg = match e {
        CredentialEffect::Remember { cwd, credential } => {
            CredentialMsg::Remembered { host: credential.host.clone(), result: git_credential::remember(&cwd, &credential) }
        }
        CredentialEffect::Forget { cwd, credential } => {
            CredentialMsg::Forgotten { host: credential.host.clone(), result: git_credential::forget(&cwd, &credential) }
        }
    };
    Msg::Credentials(msg)
}

impl Execute for CredentialEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}
