//! The files a pull request changes, from its local remote-tracking tips (Effect::LoadPrFiles).

use crate::msg::Msg;
use crate::pull_request::{FilesJob, PullRequestMsg};
use trellis_core::repo;

pub(super) fn run(job: FilesJob) -> Msg {
    let result = repo::branch_changes(&job.workdir, job.tips.0, job.tips.1).map_err(|e| e.to_string());
    Msg::PullRequest(PullRequestMsg::FilesLoaded { id: job.id, tips: job.tips, generation: job.generation, result })
}
