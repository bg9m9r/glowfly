//! A stack push job (`Effect::Stack`, runner/stack.rs).

use glowfly_core::stack::PushTarget;
use std::path::PathBuf;

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

impl StackEffect {
    pub fn name(&self) -> &'static str {
        match self {
            StackEffect::Push { .. } => "stack-push",
        }
    }
}
