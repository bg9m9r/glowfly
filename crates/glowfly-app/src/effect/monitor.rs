//! The job of `Effect::TurnOnMonitor`: git's file system monitor started and set
//! (`repo::turn_on_monitor`), then the cache's commands (runner/monitor.rs).

#[cfg(test)]
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
    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
