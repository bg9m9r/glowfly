//! Messages of the repository trust prompt (update/trust.rs), of passing trust on to the
//! repositories Glowfly makes from a trusted one (update/trust_inherit.rs) and of the runner's
//! re-check finding a repository no longer covered just before work started (update/trust_seal.rs).

use super::Inactive;
use crate::state::trust::InheritKind;
use glowfly_core::trust::{TrustPrint, TrustScan};
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
    /// The runner didn't start `Stale::effect`: its repository's scan just before it would
    /// have run isn't covered by the decision it was sent under (update/trust_seal.rs).
    Stale(Box<Stale>),
}

/// A checked effect that didn't start (`TrustMsg::Stale`).
pub struct Stale {
    pub check: crate::effect::TrustCheck,
    pub effect: crate::effect::Effect,
    /// The repository's own scan just before it would have run (`trust::scan_now`).
    pub now: TrustScan,
    /// What the commit it brings in has where hooks follow the working tree, when that is what
    /// isn't covered (`trust::follow::incoming`).
    pub incoming: Option<TrustScan>,
    /// Why what it brings in couldn't be compared, when that is why it didn't start.
    pub unchecked: Option<crate::effect::Accepted>,
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
    /// as it is a user intent or an unscoped result (`Stale` too) and never comes back scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Decide { .. } | Self::Ask { .. } | Self::AskForTerminal { .. } | Self::Inherited { .. } | Self::Stale(_) => None,
        }
    }
}
