//! A stack push job (`Effect::Stack`, runner/stack.rs).

use super::EffectMeta;
use super::SandboxRule;
use glowfly_core::stack::PushTarget;
use std::path::{Path, PathBuf};

/// A stack push job: the layers' targets, and the branch whose stack's pull requests start
/// once it lands.
#[derive(Debug, Clone)]
pub struct StackPushJob {
    pub targets: Vec<PushTarget>,
    pub then_prs: Option<String>,
}

#[derive(Debug)]
pub enum StackEffect {
    /// Pushes the layers (`StackMsg::Pushed`); a job, unscoped like RunGit.
    Push { cwd: PathBuf, job: StackPushJob },
}

impl EffectMeta for StackEffect {
    fn name(&self) -> &'static str {
        match self {
            StackEffect::Push { .. } => "stack-push",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    /// Pushes update remote-tracking refs at most, like a push RunGit (`network_only`).
    fn queue(&self) -> Option<&Path> {
        None
    }

    /// Journeys push to local bare repositories.
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
