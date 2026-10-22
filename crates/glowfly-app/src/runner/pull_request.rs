//! The pull request panel's work (`Effect::PullRequest`): the files and commits it brings
//! (`repo::pr_compare`, from the target's remote-tracking tip to the listed head when it is
//! here, else to the remote-tracking source branch), the fetch of the service's head ref for
//! its objects (writing no ref), and its calls to the hosting service (the page, a comment, a
//! review, its checks, a merge, a thread reply or resolve). The calls sign in the way the pull
//! request list does, in the panel's repository.

use crate::effect::ForgeError;
use crate::msg::Msg;
use crate::pull_request::{CallJob, PrCall, PrEffect, PrReply, PullRequestMsg};
use glowfly_core::forge::{self, Auth, CheckQuery};
use glowfly_core::repo;

pub(super) fn run(e: PrEffect) -> Msg {
    match e {
        PrEffect::Files(job) => {
            let result = repo::pr_compare(&job.workdir, job.target, job.branch, job.head.as_deref()).map_err(|e| e.to_string());
            Msg::PullRequest(PullRequestMsg::FilesLoaded { id: job.id, generation: job.generation, result })
        }
        PrEffect::FetchHead(job) => {
            let (ok, output) = super::git_ops::run_git(&job.workdir, std::slice::from_ref(&job.cmd), None, job.prompts);
            let result = if ok { Ok(()) } else { Err(glowfly_core::logs::last_message(&output)) };
            Msg::PullRequest(PullRequestMsg::HeadFetched { id: job.id, head: job.head, result })
        }
        PrEffect::Call(job) => call(*job),
    }
}

fn call(job: CallJob) -> Msg {
    let CallJob { req, id, generation, call } = job;
    // Loading the page or the checks in the background never asks for credentials; the user's
    // own actions may.
    let interactive = !matches!(call, PrCall::Load | PrCall::Checks { .. });
    let auth: Result<Auth, ForgeError> = match req.auth {
        Some(a) => Ok(a),
        None => forge::resolve_auth(&req.target.account(), &req.workdir, interactive, req.connected).map_err(ForgeError::from),
    };
    let t = &req.target;
    let reply = match call {
        PrCall::Load => PrReply::Loaded(auth.and_then(|a| forge::pr_page(t, &a, id).map_err(ForgeError::from))),
        PrCall::Checks { head } => {
            PrReply::Checks(auth.and_then(|a| forge::check_list(t, &a, &CheckQuery { id, head }).map_err(ForgeError::from)))
        }
        PrCall::Comment(body) => PrReply::Commented(auth.and_then(|a| forge::post_comment(t, &a, id, &body).map_err(ForgeError::from))),
        PrCall::Review { event, body, pending } => {
            PrReply::Reviewed(event, auth.and_then(|a| forge::submit_review(t, &a, id, event, &body, pending).map_err(ForgeError::from)))
        }
        PrCall::Merge { req: merge, remote, source, delete } => {
            let result = auth.and_then(|a| forge::merge(t, &a, id, &merge).map_err(ForgeError::from));
            PrReply::Merged { method: merge.method, remote, source, delete, result }
        }
        PrCall::Reply { thread, comment, body } => PrReply::RepliedToThread {
            thread,
            result: auth.and_then(|a| forge::reply_to_thread(t, &a, id, comment, &body).map_err(ForgeError::from)),
        },
        PrCall::Resolve { thread, resolve } => {
            let result = auth.and_then(|a| forge::resolve_thread(t, &a, &thread, resolve).map_err(ForgeError::from));
            PrReply::Resolved { thread, resolve, result }
        }
    };
    Msg::PullRequest(PullRequestMsg::Replied { workdir: req.workdir, target: req.target, id, generation, reply: Box::new(reply) })
}
