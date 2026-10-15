//! Messages of the repository trust prompt (update/trust.rs).

use super::Inactive;
use std::path::PathBuf;

/// The repository trust prompt (update/trust.rs).
pub enum TrustMsg {
    /// Trust or don't trust the repository at `real` (`Snapshot::real_workdir`).
    Decide { real: PathBuf, trusted: bool },
    /// Show the trust prompt again for the active repository at `real` (toolbar chip, palette).
    Ask { real: PathBuf },
}

impl TrustMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as it is a user intent and never comes back scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Decide { .. } | Self::Ask { .. } => None,
        }
    }
}
