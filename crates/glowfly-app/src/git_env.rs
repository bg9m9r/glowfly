//! Messages and effects about git itself (update/git_env.rs, runner/git_env.rs,
//! ui/git_banner.rs, ui/prefs/git_program.rs): which git runs and what it can do, checked at
//! startup and shown in Preferences and, when it is missing or too old, in a banner; choosing
//! another git in Preferences > Git or, when the saved one can't run, the git on PATH from the
//! banner; and repositories git refuses because another user owns
//! them, which can be added to `safe.directory`.

use crate::effect::LoadKind;
use glowfly_core::git_program::Candidate;
use glowfly_core::git_version::GitInfo;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum GitEnvMsg {
    /// At startup: find git and ask its version.
    Detect,
    /// Hides the banner until Glowfly restarts.
    DismissBanner,
    /// The user agreed to list `root` in `safe.directory` (then it opens again).
    AddSafeDirectory(PathBuf),
    /// Open Preferences on the Git page (opening Preferences when closed) and look for gits.
    ShowPage,
    /// Look for installed gits again.
    Rescan,
    /// Use this git from now on (None: git from PATH), once it is checked.
    Choose(Option<PathBuf>),
    /// Pick a git program with a file dialog.
    Browse,
    /// The banner's offer: use the git on PATH from now on, once it is checked.
    UsePathGit,
    // ---- from the runner
    /// The check started when `generation` switches to another git had been made.
    Detected {
        generation: u64,
        info: GitInfo,
    },
    /// The PATH git, checked because the saved git can't run (None: no git on PATH).
    PathGitChecked(Option<Candidate>),
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
    /// The gits found by the scan numbered `generation`.
    Discovered {
        generation: u64,
        candidates: Vec<Candidate>,
    },
    /// What the file dialog picked (None: cancelled).
    Picked(Option<PathBuf>),
    /// The check of a chosen git: on success it is already in effect, with this info.
    Switched {
        program: Option<PathBuf>,
        result: Result<GitInfo, String>,
    },
}

impl GitEnvMsg {
    /// Only `NotOwned` is scoped (it answers a load); for an inactive tab the refusal is logged.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::NotOwned { root, .. } => Some(crate::msg::Inactive::Log(not_owned_text(&root))),
            // The rest is about git itself, the same for every tab.
            Self::Detect
            | Self::DismissBanner
            | Self::AddSafeDirectory(_)
            | Self::ShowPage
            | Self::Rescan
            | Self::Choose(_)
            | Self::Browse
            | Self::UsePathGit
            | Self::Detected { .. }
            | Self::PathGitChecked(_)
            | Self::SafeDirectoryAdded { .. }
            | Self::Discovered { .. }
            | Self::Picked(_)
            | Self::Switched { .. } => None,
        }
    }
}

/// The log line for a repository git refuses for its owner.
pub fn not_owned_text(root: &std::path::Path) -> String {
    format!(
        "Could not open {}: git won't open a repository another user owns until it is listed in safe.directory.",
        glowfly_core::native_path::shown(root)
    )
}

/// Run by runner/git_env.rs.
#[derive(Debug)]
pub enum GitEnvEffect {
    /// Find the git in effect and ask its version; `generation` is `GitState::switches` now.
    Detect {
        generation: u64,
        cwd: PathBuf,
    },
    AddSafeDirectory(PathBuf),
    /// Find and check the installed gits.
    Discover {
        generation: u64,
        cwd: PathBuf,
    },
    /// Check `program` (None: git from PATH) and, when it works, run it from now on.
    Use {
        program: Option<PathBuf>,
        cwd: PathBuf,
    },
    /// The file dialog that picks a git program.
    PickProgram,
    /// Find and check the git a bare `git` runs (the saved git can't run).
    CheckPathGit {
        cwd: PathBuf,
    },
}

impl GitEnvEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            GitEnvEffect::Detect { .. } => "detect-git",
            GitEnvEffect::AddSafeDirectory(_) => "add-safe-directory",
            GitEnvEffect::Discover { .. } => "discover-git",
            GitEnvEffect::Use { .. } => "use-git",
            GitEnvEffect::PickProgram => "pick-git",
            GitEnvEffect::CheckPathGit { .. } => "check-path-git",
        }
    }
}
