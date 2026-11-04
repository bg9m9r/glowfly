//! Effects `Runner::run` carries out itself on the calling thread, never on a worker
//! (`Effect::Control`, runner/control.rs): the file watcher it owns, the credential-prompt
//! listener the helper waits on, and the stops of running git commands. None queues behind
//! another effect, so a stop or an answer reaches the job it is for while that job runs.

use super::partial::StopKey;
use super::{EffectMeta, SandboxRule, WatchTarget};
use glowfly_core::askpass::Reply;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ControlEffect {
    /// Watch this repository for outside changes, replacing any previous watch; None stops
    /// watching (the watcher outlives a worker thread).
    Watch(Option<WatchTarget>),
    /// Check the watched tree for changes the watcher may have missed (Windows poll).
    SweepWatch { workdir: PathBuf },
    /// Answers credential prompt `id`. The answer's `Debug` output is redacted.
    AnswerPrompt { id: u64, reply: Reply },
    /// Stops the running git command registered under this key, if any.
    StopGit(StopKey),
}

impl EffectMeta for ControlEffect {
    fn name(&self) -> &'static str {
        match self {
            ControlEffect::Watch(_) => "watch",
            ControlEffect::SweepWatch { .. } => "sweep-watch",
            ControlEffect::AnswerPrompt { .. } => "answer-prompt",
            ControlEffect::StopGit(_) => "stop-git",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// They wait on the system or never run on a worker thread.
    fn timed(&self) -> bool {
        false
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
