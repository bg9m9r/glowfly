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
