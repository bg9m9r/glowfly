//! The words for the typed errors that result messages carry instead of English text (#367):
//! one function per area's error type, the only place its variants become text in the user's
//! language. Each gives the fragment that follows a headline's colon ("Couldn't save the
//! sign-in for github.com: <this>"); git's own words, when an error has them, are kept as they
//! are. Pure.

use glowfly_core::git_credential::CredentialError;
use glowfly_core::i18n::Lang;

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
