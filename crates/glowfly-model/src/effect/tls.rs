//! A TLS failure's work in the runner (`Effect::Tls`, runner/tls.rs): reading the TLS settings
//! git uses for a certificate or revocation failure's Details (`glowfly_core::tls_config`), and
//! setting `http.schannelCheckRevoke` to best-effort once confirmed.

use super::SandboxRule;
use super::meta::EffectMeta;
use crate::state::job_spec::Retry;
use crate::state::logs::EntryId;
use glowfly_core::tls_config::TlsFailure;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum TlsEffect {
    /// Reads the settings git uses in `cwd` for `url` (no credentials in it); reads only.
    /// Reports `TlsMsg::SettingsRead` for run `entry`.
    ReadSettings { cwd: PathBuf, url: Option<String>, entry: EntryId, failure: TlsFailure },
    /// Sets `http.schannelCheckRevoke` to best-effort in `cwd`'s repository, or with `global`
    /// in the global config (`cwd` outside any repository). Reports `TlsMsg::BestEffortSet`
    /// with `retry`.
    SetBestEffort { cwd: PathBuf, global: bool, retry: Option<Box<Retry>> },
}

impl EffectMeta for TlsEffect {
    fn name(&self) -> &'static str {
        match self {
            TlsEffect::ReadSettings { .. } => "read-tls-settings",
            TlsEffect::SetBestEffort { .. } => "set-check-revoke",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    /// The repository's own write waits for its other changes.
    fn queue(&self) -> Option<&Path> {
        match self {
            TlsEffect::SetBestEffort { global: false, cwd, .. } => Some(cwd),
            _ => None,
        }
    }

    /// The global write would touch the developer's own config.
    fn sandbox(&self) -> SandboxRule {
        match self {
            TlsEffect::SetBestEffort { global: true, .. } => SandboxRule::Drop,
            _ => SandboxRule::Run,
        }
    }
}
