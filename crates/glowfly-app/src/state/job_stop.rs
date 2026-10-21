//! Cancelling the active job: whether it can be stopped (every command it runs only fetches),
//! whether a stop was asked for, and whether the runner reported it stopped.

use super::AppState;

/// The active job's stop, set when it starts with a stoppable effect.
#[derive(Debug, Default)]
pub struct JobStop {
    /// Cancel was pressed; the runner was asked to stop it.
    pub requested: bool,
    /// The runner reported it stopped (`JobMsg::Stopped`): it ends as cancelled.
    pub stopped: bool,
}

impl AppState {
    /// A job runs that can be stopped and hasn't been asked to yet (the status bar's Cancel).
    pub fn job_cancellable(&self) -> bool {
        self.job.is_some() && self.job_stop.as_ref().is_some_and(|s| !s.requested)
    }

    /// The running job was asked to stop and hasn't ended yet.
    pub fn job_cancelling(&self) -> bool {
        self.job.is_some() && self.job_stop.as_ref().is_some_and(|s| s.requested)
    }
}
