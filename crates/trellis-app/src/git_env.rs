//! Messages and effects about git itself (update/git_env.rs, runner/git_env.rs,
//! ui/git_banner.rs): which git runs and what it can do, checked once at startup and shown in
//! Preferences and, when it is missing or too old, in a banner; and repositories git refuses
//! because another user owns them, which can be added to `safe.directory`.

use crate::effect::LoadKind;
use std::path::PathBuf;
use trellis_core::git_version::GitInfo;

#[derive(Clone, Debug, PartialEq)]
pub enum GitEnvMsg {
    /// At startup: find git and ask its version.
    Detect,
    /// Hides the banner until Trellis restarts.
    DismissBanner,
    /// The user agreed to list `root` in `safe.directory` (then it opens again).
    AddSafeDirectory(PathBuf),
    // ---- from the runner
    Detected(GitInfo),
    /// A load was refused because another user owns the repository at `root`.
    NotOwned {
        generation: u64,
        kind: LoadKind,
        root: PathBuf,
    },
    SafeDirectoryAdded {
        root: PathBuf,
        result: Result<(), String>,
    },
}

impl GitEnvMsg {
    /// Only `NotOwned` is scoped (it answers a load); for an inactive tab the refusal is logged.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::NotOwned { root, .. } => Some(crate::msg::Inactive::Log(not_owned_text(&root))),
            Self::Detect | Self::DismissBanner | Self::AddSafeDirectory(_) | Self::Detected(_) | Self::SafeDirectoryAdded { .. } => None,
        }
    }
}

/// The log line for a repository git refuses for its owner.
pub fn not_owned_text(root: &std::path::Path) -> String {
    format!(
        "Could not open {}: git won't open a repository another user owns until it is listed in safe.directory.",
        trellis_core::native_path::shown(root)
    )
}

/// Run by runner/git_env.rs.
#[derive(Debug)]
pub enum GitEnvEffect {
    Detect { cwd: PathBuf },
    AddSafeDirectory(PathBuf),
}

impl GitEnvEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            GitEnvEffect::Detect { .. } => "detect-git",
            GitEnvEffect::AddSafeDirectory(_) => "add-safe-directory",
        }
    }
}
