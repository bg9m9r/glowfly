//! The words for the typed errors that result messages carry instead of English text (#367):
//! one function per area's error type, the only place its variants become text in the user's
//! language. Each gives the fragment that follows a headline's colon ("Couldn't save the
//! sign-in for github.com: <this>"); git's own words, when an error has them, are kept as they
//! are. Pure.

use glowfly_core::forge::ServiceError;
use glowfly_core::git_credential::CredentialError;
use glowfly_core::i18n::Lang;
use glowfly_core::identity::ConfigWriteFailed;
use glowfly_core::signing::KeyError;

/// Why saving or dropping an HTTPS sign-in with git's credential helper failed
/// (`CredentialMsg::Remembered` and `Forgotten`).
pub fn credential(t: Lang, e: &CredentialError) -> String {
    match e {
        CredentialError::NoStore { install: Some(store) } => t.trf("credential_errors.no_store.install", &[("store", store)]),
        CredentialError::NoStore { install: None } => t.tr("credential_errors.no_store").to_owned(),
        CredentialError::SetUpFailed { said: Some(line) }
        | CredentialError::ApproveFailed { said: Some(line) }
        | CredentialError::RejectFailed { said: Some(line) } => line.clone(),
        CredentialError::SetUpFailed { said: None } => t.tr("credential_errors.set_up").to_owned(),
        CredentialError::ApproveFailed { said: None } => t.tr("credential_errors.approve").to_owned(),
        CredentialError::RejectFailed { said: None } => t.tr("credential_errors.reject").to_owned(),
    }
}

/// Why signing in to a hosting service, or a call to it, failed, in the words its error has
/// always shown: the kind, then the service's own words (and an HTTP refusal's status)
/// (`ForgeMsg::Connected`, `TrelloBoardsLoaded`, `PublishMsg::PublishOwners` and `Published`).
pub fn service(t: Lang, e: &ServiceError) -> String {
    match e {
        ServiceError::Auth { said } => t.trf("service_errors.said.auth", &[("said", said)]),
        ServiceError::Forbidden { said } => t.trf("service_errors.said.forbidden", &[("said", said)]),
        ServiceError::NotFound { said } => t.trf("service_errors.said.not_found", &[("said", said)]),
        ServiceError::Http { status, message } => t.trf("service_errors.said.http", &[("status", status), ("said", message)]),
        ServiceError::Network { said } => t.trf("service_errors.said.network", &[("said", said)]),
        ServiceError::Decode { said } => t.trf("service_errors.said.decode", &[("said", said)]),
        ServiceError::Keyring { said } => t.trf("service_errors.said.keyring", &[("said", said)]),
        ServiceError::Other { said } => said.clone(),
    }
}
/// Why listing, generating or exporting a signing key failed (`PrefsMsg::SigningKeysLoaded`,
/// `GpgKeyGenerated` and `PublicKeyExported`).
pub fn signing_key(t: Lang, e: &KeyError) -> String {
    match e {
        KeyError::NoIdentity => t.tr("signing_errors.no_identity").to_owned(),
        KeyError::NotRun { program, said } => t.trf("signing_errors.not_run", &[("program", program), ("said", said)]),
        KeyError::Failed { program, said } => t.trf("signing_errors.failed", &[("program", program), ("said", said)]),
        KeyError::GenerateFailed { said } => t.trf("signing_errors.generate_failed", &[("said", said)]),
        KeyError::NewKeyNotFound => t.tr("signing_errors.new_key_not_found").to_owned(),
        KeyError::NoPublicKey { key } => t.trf("signing_errors.no_public_key", &[("key", key)]),
        KeyError::Io { said } => said.clone(),
    }
}

/// Why a Preferences page's git config write failed (`PrefsMsg::ConfigSaved`).
pub fn config_write(t: Lang, e: &ConfigWriteFailed) -> String {
    t.trf("config_errors.write", &[("key", &e.key), ("said", &e.said)])
}
