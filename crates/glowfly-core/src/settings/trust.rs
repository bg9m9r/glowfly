//! Repository trust decisions (see `trust`): what the user answered for a repository.

use serde::{Deserialize, Serialize};

/// The user's answer to "Trust this repository?".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoTrust {
    /// Git may run the programs the repository's config and hooks name, on its own too.
    Trusted,
    /// Glowfly doesn't fetch, sign in or let git read the status on its own here.
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

    /// Whether this is Trusted and its print covers `now`. A Trusted decision without a print
    /// (saved before prints were kept, or recorded where no scan was at hand) counts as made on
    /// a scan that found nothing: it covers only what runs nothing, so it asks again once.
    pub fn covers(&self, now: &crate::trust::TrustPrint) -> bool {
        self.is_trusted() && self.print.as_ref().map_or_else(|| now.is_empty(), |p| p.covers(now))
    }
}
