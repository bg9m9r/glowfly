//! The pull request panel's work (`Effect::PullRequest`): the files it changes, from its local
//! remote-tracking tips, and its calls to the hosting service (the page, a comment, a review,
//! a merge). The calls sign in the way the pull request list does, in the panel's repository.

use crate::effect::ForgeError;
use crate::msg::Msg;
use crate::pull_request::{CallJob, PrCall, PrEffect, PrReply, PullRequestMsg};
use trellis_core::forge::{self, Auth};
use trellis_core::repo;

pub(super) fn run(e: PrEffect) -> Msg {
    match e {
        PrEffect::Files(job) => {
            let result = repo::branch_changes(&job.workdir, job.tips.0, job.tips.1).map_err(|e| e.to_string());
            Msg::PullRequest(PullRequestMsg::FilesLoaded { id: job.id, tips: job.tips, generation: job.generation, result })
        }
        PrEffect::Call(job) => call(*job),
    }
}

fn call(job: CallJob) -> Msg {
    let CallJob { req, id, generation, call } = job;
    // Loading the page in the background never asks for credentials; the user's own actions may.
    let interactive = !matches!(call, PrCall::Load);
    let auth: Result<Auth, ForgeError> = match req.auth {
        Some(a) => Ok(a),
        None => forge::resolve_auth(&req.target.account(), &req.workdir, interactive, req.connected).map_err(ForgeError::from),
    };
    let t = &req.target;
    let reply = match call {
        PrCall::Load => PrReply::Loaded(auth.and_then(|a| forge::pr_page(t, &a, id).map_err(ForgeError::from))),
        PrCall::Comment(body) => PrReply::Commented(auth.and_then(|a| forge::post_comment(t, &a, id, &body).map_err(ForgeError::from))),
        PrCall::Review { event, body, pending } => {
            PrReply::Reviewed(event, auth.and_then(|a| forge::submit_review(t, &a, id, event, &body, pending).map_err(ForgeError::from)))
        }
        PrCall::Merge { req: merge, remote, source, delete } => {
            let result = auth.and_then(|a| forge::merge(t, &a, id, &merge).map_err(ForgeError::from));
            PrReply::Merged { method: merge.method, remote, source, delete, result }
        }
    };
    Msg::PullRequest(PullRequestMsg::Replied { workdir: req.workdir, target: req.target, id, generation, reply: Box::new(reply) })
}
