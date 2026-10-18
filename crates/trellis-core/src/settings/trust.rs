//! Repository trust decisions (see `trust`): what the user answered for a repository.

use serde::{Deserialize, Serialize};

/// The user's answer to "Trust this repository?".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoTrust {
    /// Git may run the programs the repository's config and hooks name, on its own too.
    Trusted,
    /// Trellis doesn't fetch, sign in or let git read the status on its own here.
    NotTrusted,
}

/// A saved decision with what it was made on.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedTrust {
    pub trust: RepoTrust,
    /// The print of the scan a Trusted decision was made on (`trust::TrustPrint`); None for
    /// Don't trust, and for decisions saved before prints were kept.
    pub print: Option<crate::trust::TrustPrint>,
}

impl SavedTrust {
    /// Trusted, made on `print`.
    pub fn trusted(print: Option<crate::trust::TrustPrint>) -> Self {
        Self { trust: RepoTrust::Trusted, print }
    }

    pub fn is_trusted(&self) -> bool {
        self.trust == RepoTrust::Trusted
    }
}
