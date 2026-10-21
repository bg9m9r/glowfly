//! Saving and dropping HTTPS sign-ins with git's credential helpers (`CredentialEffect`).

use crate::effect::CredentialEffect;
use crate::msg::{CredentialMsg, Msg};
use glowfly_core::git_credential;

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
