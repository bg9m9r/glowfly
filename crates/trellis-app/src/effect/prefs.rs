//! Effects behind the Preferences pages: reading and writing git config values, and listing,
//! generating and exporting signing keys (`Effect::Prefs`, run by runner/config.rs and
//! runner/keys.rs). Unscoped.

use std::path::PathBuf;
use trellis_core::signing::SigningFormat;

#[derive(Debug)]
pub enum PrefsEffect {
    ReadGitConfig { cwd: PathBuf, has_repo: bool },
    SetGitConfig { cwd: PathBuf, global: bool, key: String, value: Option<String> },
    ListSigningKeys { format: SigningFormat, program: String },
    GenerateGpgKey { program: String, name: String, email: String, passphrase: String },
    ExportPublicKey { format: SigningFormat, program: String, key: String },
}

impl PrefsEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            PrefsEffect::ReadGitConfig { .. } => "read-config",
            PrefsEffect::SetGitConfig { .. } => "set-config",
            PrefsEffect::ListSigningKeys { .. } => "list-keys",
            PrefsEffect::GenerateGpgKey { .. } => "generate-key",
            PrefsEffect::ExportPublicKey { .. } => "export-key",
        }
    }

    /// Whether its run time goes to the performance log: generating a key waits on the user's
    /// passphrase dialog, so it doesn't.
    pub fn timed(&self) -> bool {
        !matches!(self, PrefsEffect::GenerateGpgKey { .. })
    }
}
