//! Messages of the repository trust prompt (update/trust.rs) and of passing trust on to the
//! repositories Trellis makes from a trusted one (update/trust_inherit.rs).

use super::Inactive;
use crate::state::trust::InheritKind;
use glowfly_core::trust::TrustPrint;
use std::path::PathBuf;

/// The repository trust prompt (update/trust.rs).
pub enum TrustMsg {
    /// Trust or don't trust the repository at `real` (`Snapshot::real_workdir`).
    Decide { real: PathBuf, trusted: bool },
    /// Show the trust prompt again for the active repository at `real` (toolbar chip, palette).
    Ask { real: PathBuf },
    /// Show the trust prompt for the active repository at `real` on behalf of its held terminal
    /// shells (the panel's Trust and run…).
    AskForTerminal { real: PathBuf },
    /// The repositories a job just made from the trusted repository `from` (its real path), as
    /// read by the runner; `open` opens once they are recorded.
    Inherited { kind: InheritKind, from: PathBuf, repos: Vec<InheritedRepo>, open: Option<PathBuf> },
}

/// One repository `TrustMsg::Inherited` reports.
#[derive(Debug, PartialEq)]
pub struct InheritedRepo {
    /// The folder as asked.
    pub path: PathBuf,
    /// Where it really is on disk: the key a decision is saved under.
    pub real: PathBuf,
    /// The print of its scan; None when it isn't a repository.
    pub print: Option<TrustPrint>,
}

impl TrustMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as it is a user intent or an unscoped result and never comes back scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Decide { .. } | Self::Ask { .. } | Self::AskForTerminal { .. } | Self::Inherited { .. } => None,
        }
    }
}
