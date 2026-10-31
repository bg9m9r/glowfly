//! Signing keys for the Preferences signing page: listing, generating and exporting them.

use crate::msg::{Msg, PrefsMsg};
use glowfly_core::signing::{self, SigningFormat};

pub(super) fn list(format: SigningFormat, program: &str) -> Msg {
    let now = chrono::Utc::now().timestamp();
    let result = match format {
        SigningFormat::Ssh => Ok(signing::home_dir().map(|h| signing::list_ssh_keys(&h)).unwrap_or_default()),
        SigningFormat::OpenPgp => signing::list_gpg_keys(program, now).map_err(|e| e.to_string()),
        // X.509 certificates are chosen by id; listing them through gpgsm isn't supported yet.
        SigningFormat::X509 => Ok(Vec::new()),
    };
    Msg::Prefs(PrefsMsg::SigningKeysLoaded(result))
}

pub(super) fn generate_gpg(program: &str, name: &str, email: &str, passphrase: &str) -> Msg {
    let now = chrono::Utc::now().timestamp();
    Msg::Prefs(PrefsMsg::GpgKeyGenerated(signing::generate_gpg_key(program, name, email, passphrase, now).map_err(|e| e.to_string())))
}

pub(super) fn export_public(format: SigningFormat, program: &str, key: &str) -> Msg {
    Msg::Prefs(PrefsMsg::PublicKeyExported(signing::export_public_key(format, program, key).map_err(|e| e.to_string())))
}
