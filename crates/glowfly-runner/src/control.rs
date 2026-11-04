//! `ControlEffect`s, which `Runner::run` carries out itself on the calling thread: setting and
//! sweeping the file watcher it owns, answering a credential prompt through its listener, and
//! asking a running git command to stop. None is queued or handed to a worker, so a stop or an
//! answer reaches the job it is for while that job is still running (or waiting in its queue).

use super::Runner;
use super::execute::{Cx, Execute};
use glowfly_model::effect::ControlEffect;
use glowfly_model::msg::Msg;
use std::path::PathBuf;

impl Runner {
    /// Carries out `effect` before returning.
    pub(super) fn control(&self, effect: ControlEffect) {
        match effect {
            ControlEffect::Watch(target) => self.watch.set(target, self.tx.clone(), self.waker.clone()),
            ControlEffect::SweepWatch { workdir } => self.sweep_watch(workdir),
            ControlEffect::AnswerPrompt { id, reply } => self.answer_prompt(id, reply),
            ControlEffect::StopGit(key) => {
                let asked = self.stops.stop(key);
                tracing::debug!(?key, asked, "stop git");
            }
        }
    }

    /// Has the watcher of `workdir` sweep its tree; with none running there, reads the whole
    /// status instead (as the poll did before the sweep).
    fn sweep_watch(&self, workdir: PathBuf) {
        if self.watch.sweep(&workdir) {
            return;
        }
        let msg = glowfly_model::msg::WatchMsg::RepoChanged { full: false, paths: None, last_event: std::time::Instant::now() };
        if self.tx.send(Msg::Scoped { workdir, msg: Box::new(Msg::Watch(msg)) }).is_ok() {
            self.waker.wake();
        }
    }
}

/// Never reached: `Runner::run` carries these out before any worker starts.
impl Execute for ControlEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        match self {
            // There is nothing running here to stop.
            ControlEffect::StopGit(_) => None,
            ControlEffect::Watch(_) | ControlEffect::SweepWatch { .. } | ControlEffect::AnswerPrompt { .. } => {
                tracing::warn!("watch or prompt effect reached a worker; it is handled by Runner::run");
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
