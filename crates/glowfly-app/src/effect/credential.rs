//! Saving and dropping HTTPS sign-ins typed in the credential prompt (runner/credentials.rs).

use glowfly_core::git_credential::HttpsCredential;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CredentialEffect {
    /// Git accepted the sign-in: save it with git's credential helper (`git credential approve`),
    /// setting up the platform's store when git has none.
    Remember { cwd: PathBuf, credential: HttpsCredential },
    /// The server rejected the sign-in: have the helper drop it (`git credential reject`).
    Forget { cwd: PathBuf, credential: HttpsCredential },
}

impl CredentialEffect {
    pub fn name(&self) -> &'static str {
        match self {
            CredentialEffect::Remember { .. } => "remember-credential",
            CredentialEffect::Forget { .. } => "forget-credential",
        }
    }
}
