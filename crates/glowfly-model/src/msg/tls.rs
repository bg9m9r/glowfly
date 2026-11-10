//! Messages of a TLS failure's help (update/tls_fix.rs): the settings read for a certificate or
//! revocation failure's Details, and Relax check (`http.schannelCheckRevoke` set to
//! best-effort) once confirmed, with its result.

use super::Inactive;
use crate::state::job_spec::Retry;
use crate::state::logs::EntryId;
use glowfly_core::error::ReadFailed;
use glowfly_core::identity::ConfigWriteFailed;
use glowfly_core::tls_config::{TlsFailure, TlsSettings};
use std::path::PathBuf;

pub enum TlsMsg {
    /// The runner read the TLS settings git uses (runner/tls.rs) for failed run `entry`.
    SettingsRead { entry: EntryId, failure: TlsFailure, result: Result<Box<TlsSettings>, ReadFailed> },
    /// Relax check, confirmed: set best-effort in `workdir`'s settings, or with `global` in the
    /// global ones (`workdir` is then `AppState::outside_dir()`), then run `retry`.
    SetBestEffort { workdir: PathBuf, global: bool, retry: Option<Box<Retry>> },
    /// The write ended; on success `retry` runs.
    BestEffortSet { global: bool, retry: Option<Box<Retry>>, result: Result<(), ConfigWriteFailed> },
}

impl TlsMsg {
    /// Its effects are unscoped, so results always reach `update`; the settings note is kept by
    /// `EntryId` in the global `LogsState`, whichever tab is active.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::SettingsRead { .. } | Self::SetBestEffort { .. } | Self::BestEffortSet { .. } => None,
        }
    }
}
