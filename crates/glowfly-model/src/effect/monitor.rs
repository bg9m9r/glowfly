//! The jobs of `Effect::TurnOnMonitor`: git's file system monitor started and set
//! (`repo::turn_on_monitor`), then the cache's commands, and of `Effect::TurnOffMonitor`:
//! core.fsmonitor unset and the daemon stopped (`repo::turn_off_monitor`) (runner/monitor.rs).

use super::SandboxRule;
use super::meta::EffectMeta;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct MonitorJob {
    pub cwd: PathBuf,
    /// The commands run once the monitor is on (the cache's).
    pub then: Vec<Vec<String>>,
}

impl EffectMeta for MonitorJob {
    fn name(&self) -> &'static str {
        "turn-on-monitor"
    }

    /// Unscoped like RunGit: it ends the job.
    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    /// It writes the repository's config and its index.
    fn queue(&self) -> Option<&Path> {
        Some(&self.cwd)
    }

    /// Starts a git daemon that outlives the run.
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}

/// Repository > Hooks' Turn off for git's file system monitor, in `cwd`.
#[derive(Debug)]
pub struct MonitorOff {
    pub cwd: PathBuf,
}

impl EffectMeta for MonitorOff {
    fn name(&self) -> &'static str {
        "turn-off-monitor"
    }

    /// Unscoped like RunGit: it ends the job.
    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    /// It writes the repository's config.
    fn queue(&self) -> Option<&Path> {
        Some(&self.cwd)
    }

    /// Stops a daemon some other process may own.
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
