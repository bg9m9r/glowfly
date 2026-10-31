//! Effects behind the Preferences pages: reading and writing git config values (one at a time,
//! or a page's save as one batch that reports back, `WriteGitConfig`), and listing,
//! generating and exporting signing keys, and reading the settings file again after it couldn't
//! be read (`Effect::Prefs`, run by runner/config.rs and runner/keys.rs). Unscoped.

use super::EffectMeta;
use super::SandboxRule;
use glowfly_core::signing::SigningFormat;
use std::path::{Path, PathBuf};

/// One git config value a save writes; None unsets it.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigWrite {
    pub global: bool,
    pub key: String,
    pub value: Option<String>,
}

/// `SetGitConfig` writes one value, a failure going to the activity log; `WriteGitConfig` writes
/// a Preferences save's values in order, stopping at the first that fails, and reports back as
/// `PrefsMsg::ConfigSaved` with `ticket`.
#[derive(Debug)]
pub enum PrefsEffect {
    ReadGitConfig {
        cwd: PathBuf,
        has_repo: bool,
    },
    SetGitConfig {
        cwd: PathBuf,
        global: bool,
        key: String,
        value: Option<String>,
    },
    WriteGitConfig {
        cwd: PathBuf,
        ticket: u64,
        writes: Vec<ConfigWrite>,
    },
    ListSigningKeys {
        format: SigningFormat,
        program: String,
    },
    GenerateGpgKey {
        program: String,
        name: String,
        email: String,
        passphrase: String,
    },
    ExportPublicKey {
        format: SigningFormat,
        program: String,
        key: String,
    },
    /// Reads `settings.json` again (`Settings::reload`), reporting `PrefsMsg::SettingsReread`.
    ReadSettings,
}

impl EffectMeta for PrefsEffect {
    /// A short name for the worker thread and the debug log.
    fn name(&self) -> &'static str {
        match self {
            PrefsEffect::ReadGitConfig { .. } => "read-config",
            PrefsEffect::SetGitConfig { .. } => "set-config",
            PrefsEffect::WriteGitConfig { .. } => "write-config",
            PrefsEffect::ListSigningKeys { .. } => "list-keys",
            PrefsEffect::GenerateGpgKey { .. } => "generate-key",
            PrefsEffect::ExportPublicKey { .. } => "export-key",
            PrefsEffect::ReadSettings => "read-settings",
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
            | PrefsEffect::WriteGitConfig { .. }
            | PrefsEffect::ListSigningKeys { .. }
            | PrefsEffect::ExportPublicKey { .. }
            | PrefsEffect::ReadSettings => true,
            PrefsEffect::GenerateGpgKey { .. } => false,
        }
    }

    /// Git config, which no queued job reads mid-way.
    fn queue(&self) -> Option<&Path> {
        None
    }

    /// A repository's own config runs; the global config, the signing keys and the app's own
    /// settings file are dropped.
    fn sandbox(&self) -> SandboxRule {
        match self {
            PrefsEffect::ReadGitConfig { .. } | PrefsEffect::SetGitConfig { global: false, .. } => SandboxRule::Run,
            PrefsEffect::WriteGitConfig { writes, .. } if writes.iter().all(|w| !w.global) => SandboxRule::Run,
            PrefsEffect::WriteGitConfig { .. } => SandboxRule::Drop,
            PrefsEffect::SetGitConfig { global: true, .. }
            | PrefsEffect::ListSigningKeys { .. }
            | PrefsEffect::GenerateGpgKey { .. }
            | PrefsEffect::ExportPublicKey { .. }
            | PrefsEffect::ReadSettings => SandboxRule::Drop,
        }
    }
}
