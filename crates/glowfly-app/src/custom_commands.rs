//! Messages and effects of custom commands, the git and shell actions defined in the user's and
//! the repository's `commands.toml` (glowfly_core::custom_commands): the loaded lists, their
//! keys and the menus' items in state/custom_commands.rs, the palette's entries in
//! state/palette_custom.rs, loading in update/custom_commands.rs, running in
//! update/custom_commands_run.rs, the runner's side in runner/custom_commands.rs, the menus'
//! Commands submenu in ui/custom_menu.rs and the keys in ui/shortcuts/custom_keys.rs.

use crate::effect::EffectMeta;
#[cfg(test)]
use crate::effect::SandboxRule;
use glowfly_core::custom_commands::{FileRead, Source};
use std::path::{Path, PathBuf};

/// A command by the file it came from and its name, found again when it runs.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandRef {
    pub source: Source,
    pub name: String,
}

/// Where a command runs from, which decides what its placeholders hold.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// The palette or a key: the selected commit or HEAD, the open diff's file.
    Palette,
    /// A graph row's commit menu.
    Commit(git2::Oid),
    /// A local branch's menu.
    Branch(String),
    /// A file row's menu: the file and the row's commit (None for a working-tree row: HEAD).
    File { path: String, at: Option<git2::Oid> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CustomCommandMsg {
    /// Read both commands files again (the palette).
    Reload,
    /// Both files as read for `workdir` by the load numbered `generation` (runner).
    Loaded { generation: u64, workdir: PathBuf, user: FileRead, repo: FileRead },
    /// Run this command from `at`; `confirmed` once its confirmation was accepted.
    Run { command: CommandRef, at: Target, confirmed: bool },
}

impl CustomCommandMsg {
    /// Never scoped: a load names its folder and generation, which `update` checks; a run comes
    /// from the active tab.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Reload | Self::Loaded { .. } | Self::Run { .. } => None,
        }
    }
}

/// A shell command to run as the job: the repository's top folder and the filled-in script.
#[derive(Debug)]
pub struct ShellRun {
    pub cwd: PathBuf,
    pub script: String,
}

/// Reading the commands files, and running a shell command (runner/custom_commands.rs).
#[derive(Debug)]
pub enum CustomCommandEffect {
    /// Reads the user's file and `workdir`'s `.glowfly/commands.toml`.
    Load { generation: u64, workdir: PathBuf },
    /// Runs a shell command as the active job; ends with `JobMsg::JobDone`.
    RunShell(ShellRun),
}

impl EffectMeta for CustomCommandEffect {
    /// A short name for the worker thread and the debug log.
    fn name(&self) -> &'static str {
        match self {
            Self::Load { .. } => "load-custom-commands",
            Self::RunShell(_) => "run-custom-command",
        }
    }

    /// Jobs report to whichever tab is active, like RunGit.
    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Both are timed, as git jobs are.
    fn timed(&self) -> bool {
        match self {
            Self::Load { .. } | Self::RunShell(_) => true,
        }
    }

    /// A script may change the index, files and refs.
    fn queue(&self) -> Option<&Path> {
        match self {
            Self::RunShell(r) => Some(&r.cwd),
            Self::Load { .. } => None,
        }
    }

    /// Reads the user's config folder; runs the OS shell.
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
