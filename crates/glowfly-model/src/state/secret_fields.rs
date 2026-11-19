//! Preferences' secret fields (`ViewState::secrets`): the Integrations token, the GPG key's
//! passphrase and the SSH key's passphrase with its confirmation. The views edit them in place
//! and update reads and wipes them, so a secret never travels in a message (like the credential
//! prompt's `PromptFields`). Opening or closing Preferences wipes them all.

use glowfly_core::askpass;

/// The secret fields Preferences' pages and the Generate SSH key dialog draw.
#[derive(Default)]
pub struct SecretFields {
    /// Integrations' (and the tracker page's) personal access token.
    pub int_token: String,
    /// Commit Signing's passphrase for a key it generates.
    pub signing_passphrase: String,
    /// Generate SSH key's passphrase and its Confirm Passphrase field.
    pub ssh_passphrase: String,
    pub ssh_confirm: String,
}

impl SecretFields {
    /// Wipes every field.
    pub fn clear(&mut self) {
        askpass::wipe(&mut self.int_token);
        askpass::wipe(&mut self.signing_passphrase);
        self.clear_ssh();
    }

    /// Wipes the SSH key's passphrase and its confirmation.
    pub fn clear_ssh(&mut self) {
        askpass::wipe(&mut self.ssh_passphrase);
        askpass::wipe(&mut self.ssh_confirm);
    }

    /// Wipes the token (once it was used, or the sign-in form reset).
    pub fn clear_token(&mut self) {
        askpass::wipe(&mut self.int_token);
    }

    /// The signing passphrase, taken out for the key generation that uses it.
    pub fn take_signing_passphrase(&mut self) -> String {
        std::mem::take(&mut self.signing_passphrase)
    }

    /// The SSH passphrase and its confirmation differ.
    pub fn ssh_mismatch(&self) -> bool {
        self.ssh_passphrase != self.ssh_confirm
    }
}

/// Runs [`SecretFields::clear`], so the text is overwritten whenever the fields go away.
impl Drop for SecretFields {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_wipes_every_field() {
        let mut f =
            SecretFields { int_token: "t".into(), signing_passphrase: "p".into(), ssh_passphrase: "s".into(), ssh_confirm: "s".into() };
        f.clear();
        assert!(f.int_token.is_empty() && f.signing_passphrase.is_empty() && f.ssh_passphrase.is_empty() && f.ssh_confirm.is_empty());
    }
}
