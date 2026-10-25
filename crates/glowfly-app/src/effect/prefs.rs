//! Effects behind the Preferences pages: reading and writing git config values, and listing,
//! generating and exporting signing keys (`Effect::Prefs`, run by runner/config.rs and
//! runner/keys.rs). Unscoped.

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use glowfly_core::signing::SigningFormat;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PrefsEffect {
    ReadGitConfig { cwd: PathBuf, has_repo: bool },
    SetGitConfig { cwd: PathBuf, global: bool, key: String, value: Option<String> },
    ListSigningKeys { format: SigningFormat, program: String },
    GenerateGpgKey { program: String, name: String, email: String, passphrase: String },
    ExportPublicKey { format: SigningFormat, program: String, key: String },
}

impl EffectMeta for PrefsEffect {
    /// A short name for the worker thread and the debug log.
    fn name(&self) -> &'static str {
        match self {
            PrefsEffect::ReadGitConfig { .. } => "read-config",
            PrefsEffect::SetGitConfig { .. } => "set-config",
            PrefsEffect::ListSigningKeys { .. } => "list-keys",
            PrefsEffect::GenerateGpgKey { .. } => "generate-key",
            PrefsEffect::ExportPublicKey { .. } => "export-key",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Whether its run time goes to the performance log: generating a key waits on the user's
    /// passphrase dialog, so it doesn't.
    fn timed(&self) -> bool {
        match self {
            PrefsEffect::ReadGitConfig { .. }
            | PrefsEffect::SetGitConfig { .. }
            | PrefsEffect::ListSigningKeys { .. }
            | PrefsEffect::ExportPublicKey { .. } => true,
            PrefsEffect::GenerateGpgKey { .. } => false,
        }
    }

    /// Git config, which no queued job reads mid-way.
    fn queue(&self) -> Option<&Path> {
        None
    }

    /// A repository's own config runs; the global config and the signing keys are dropped.
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        match self {
            PrefsEffect::ReadGitConfig { .. } | PrefsEffect::SetGitConfig { global: false, .. } => SandboxRule::Run,
            PrefsEffect::SetGitConfig { global: true, .. }
            | PrefsEffect::ListSigningKeys { .. }
            | PrefsEffect::GenerateGpgKey { .. }
            | PrefsEffect::ExportPublicKey { .. } => SandboxRule::Drop,
        }
    }
}
